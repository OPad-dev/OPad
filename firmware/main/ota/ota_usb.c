#include "ota/ota_usb.h"

#include <string.h>
#include "esp_log.h"
#include "esp_ota_ops.h"
#include "esp_partition.h"
#include "esp_timer.h"
#include "mbedtls/sha256.h"
#include "runtime/runtime.h"
#include "ui/flash_anim.h"
#include "ui/ui.h"

static const char *TAG = "ota_usb";

// A host that stops sending for this long has gone (closed, unplugged)
#define OTA_IDLE_TIMEOUT_US (10 * 1000 * 1000)
// Erased as the writes reach it (ota_usb_chunk)
#define ERASE_BLOCK (64 * 1024)

static struct {
    bool active;
    bool animating;
    uint32_t erased;  // bytes of the slot erased so far, from its start
    const esp_partition_t *part;
    uint32_t size;
    uint32_t written;
    uint8_t sha256[32];
    mbedtls_sha256_context sha;
    int64_t last_us;
} s;

static ota_usb_reply_t reply(opad_OtaStage stage, const char *message)
{
    return (ota_usb_reply_t){.stage = stage, .written = s.written, .message = message};
}

// Everything back as it was: the running image untouched, the screen to LVGL
static void finish_failed(void)
{
    if (s.active) {
        mbedtls_sha256_free(&s.sha);
        s.active = false;
    }
    if (s.animating) {
        flash_anim_stop(false);
        s.animating = false;
    } else {
        ui_clear_notice();
    }
}

static ota_usb_reply_t fail(const char *why)
{
    ESP_LOGW(TAG, "Update over USB stopped: %s", why);
    finish_failed();
    return reply(opad_OtaStage_OTA_STAGE_ERROR, why);
}

static bool running_image_confirmed(void)
{
    esp_ota_img_states_t st;
    return esp_ota_get_state_partition(esp_ota_get_running_partition(), &st) != ESP_OK ||
           st != ESP_OTA_IMG_PENDING_VERIFY;
}

ota_usb_reply_t ota_usb_begin(uint32_t image_size, const uint8_t *sha256, size_t sha256_len)
{
    if (s.active) finish_failed();  // a host that starts over
    s.written = 0;
    if (runtime_get_state() != OPAD_STATE_IDLE) {
        return reply(opad_OtaStage_OTA_STAGE_ERROR, "not idle: a map is being played");
    }
    // Updating from an image still on trial would leave neither one to fall back to
    if (!running_image_confirmed()) {
        return reply(opad_OtaStage_OTA_STAGE_ERROR, "the running image is not confirmed yet");
    }
    if (sha256_len != sizeof(s.sha256)) {
        return reply(opad_OtaStage_OTA_STAGE_ERROR, "no SHA-256");
    }
    s.part = esp_ota_get_next_update_partition(NULL);
    if (!s.part) {
        return reply(opad_OtaStage_OTA_STAGE_ERROR, "no other app slot");
    }
    if (image_size == 0 || image_size > s.part->size) {
        return reply(opad_OtaStage_OTA_STAGE_ERROR, "image does not fit the slot");
    }
    s.animating = flash_anim_start();
    if (s.animating) {
        flash_anim_run();
    } else {
        ui_show_notice("Updating the firmware.\nKeep the cable in.");
    }
    // Written straight to the partition, not through esp_ota_begin/write:
    // those erase either the whole image's room first (2.9 s for 1 MB, with
    // the animation stalled) or a 4 KB sector per piece (~45 ms with both
    // cores stopped, all through the transfer). Here a 64 KB block is erased
    // when the writes reach it: a short stall every 16 pieces.
    // esp_ota_set_boot_partition validates the image at the end, as
    // esp_ota_end would.
    s.erased = 0;
    s.active = true;
    s.size = image_size;
    memcpy(s.sha256, sha256, sizeof(s.sha256));
    mbedtls_sha256_init(&s.sha);
    mbedtls_sha256_starts(&s.sha, 0);
    s.last_us = esp_timer_get_time();
    ESP_LOGI(TAG, "Writing %lu bytes to %s", (unsigned long)image_size, s.part->label);
    return reply(opad_OtaStage_OTA_STAGE_READY, NULL);
}

ota_usb_reply_t ota_usb_chunk(uint32_t offset, const uint8_t *data, size_t len)
{
    if (!s.active) return reply(opad_OtaStage_OTA_STAGE_ERROR, "no update in progress");
    // A piece before this one was lost on the way: the host starts again
    // from what is written. Nothing is written from this one.
    if (offset != s.written) {
        s.last_us = esp_timer_get_time();
        return reply(opad_OtaStage_OTA_STAGE_RESEND, NULL);
    }
    if (len == 0 || len > s.size - s.written) return fail("more data than the image size");
    esp_err_t err = ESP_OK;
    while (err == ESP_OK && s.written + len > s.erased) {
        uint32_t block = s.part->size - s.erased < ERASE_BLOCK ? s.part->size - s.erased : ERASE_BLOCK;
        err = esp_partition_erase_range(s.part, s.erased, block);
        s.erased += block;
    }
    if (err == ESP_OK) err = esp_partition_write(s.part, s.written, data, len);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "flash write at %lu: %s", (unsigned long)s.written, esp_err_to_name(err));
        return fail("flash write failed");
    }
    mbedtls_sha256_update(&s.sha, data, len);
    s.written += len;
    s.last_us = esp_timer_get_time();
    return reply(opad_OtaStage_OTA_STAGE_WRITTEN, NULL);
}

static void before_restart(void)
{
    // The panel keeps this frame, and the new image carries on after it
    flash_anim_stop(true);
}

ota_usb_reply_t ota_usb_end(void)
{
    if (!s.active) return reply(opad_OtaStage_OTA_STAGE_ERROR, "no update in progress");
    if (s.written != s.size) return fail("image cut short");
    uint8_t got[32];
    mbedtls_sha256_finish(&s.sha, got);
    mbedtls_sha256_free(&s.sha);
    s.active = false;
    if (memcmp(got, s.sha256, sizeof(got)) != 0) {
        return fail("SHA-256 does not match");
    }
    // Checks the image itself (header, chip, checksum) and makes it boot once
    // on trial: confirmed only when the host configures USB (app_main.c),
    // else the bootloader rolls back to the running image
    esp_err_t err = esp_ota_set_boot_partition(s.part);
    if (err == ESP_ERR_OTA_VALIDATE_FAILED) {
        return fail("not a valid image for this pad");
    }
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "esp_ota_set_boot_partition: %s", esp_err_to_name(err));
        return fail("could not select the new image");
    }
    ESP_LOGI(TAG, "%s verified; restarting into it", s.part->label);
    // Counters, config and layouts are saved first, by the runtime task
    runtime_request_restart_then(s.animating ? before_restart : NULL);
    return reply(opad_OtaStage_OTA_STAGE_DONE, NULL);
}

void ota_usb_abort(void)
{
    if (s.active) fail("cancelled by the host");
}

bool ota_usb_poll(ota_usb_reply_t *out)
{
    if (!s.active) return false;
    if (runtime_get_state() != OPAD_STATE_IDLE) {
        // No flash writes in a map: the pad is a keyboard first
        *out = fail("a map started");
        return true;
    }
    if (esp_timer_get_time() - s.last_us > OTA_IDLE_TIMEOUT_US) {
        *out = fail("the host stopped sending");
        return true;
    }
    return false;
}

bool ota_usb_active(void)
{
    return s.active;
}
