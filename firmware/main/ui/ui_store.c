#include "ui_store.h"
#include "counters/counters.h"
#include "runtime/runtime.h"
#include "diag/diag.h"
#include "nvs.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/semphr.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *TAG = "ui_store";
static const char *NVS_NAMESPACE = "osupad_ui";

// Written by the protocol task (set_layout / reset_layout while not IDLE),
// flushed by the runtime supervisor on the other core: guarded by store_lock(),
// which is also held across every layout NVS write so an older deferred layout
// can never land on top of a newer one
static ui_layout_t s_pending_layouts[UI_SCREEN_COUNT];
static uint8_t s_dirty_save_mask = 0;
static uint8_t s_dirty_erase_mask = 0;
// A failed deferred write is kept and retried, but not every 100 ms
#define FLUSH_RETRY_US 10000000
static int64_t s_flush_retry_at_us = 0;

static SemaphoreHandle_t store_lock(void)
{
    static StaticSemaphore_t buf;
    static SemaphoreHandle_t handle = NULL;
    static portMUX_TYPE init_mux = portMUX_INITIALIZER_UNLOCKED;
    portENTER_CRITICAL(&init_mux);
    if (handle == NULL) {
        handle = xSemaphoreCreateMutexStatic(&buf);
    }
    portEXIT_CRITICAL(&init_mux);
    return handle;
}

// Bump when ui_layout_t changes shape; older blobs are then ignored
#define LAYOUT_BLOB_VERSION 1

typedef struct {
    uint32_t version;
    uint32_t size;
    ui_layout_t layout;
} layout_blob_t;

static void key_for(uint8_t screen, char *key, size_t len)
{
    snprintf(key, len, "layout%u", screen);
}

bool ui_store_load(uint8_t screen, ui_layout_t *out)
{
    nvs_handle_t h;
    if (nvs_open(NVS_NAMESPACE, NVS_READONLY, &h) != ESP_OK) {
        return false;
    }
    char key[16];
    key_for(screen, key, sizeof(key));
    layout_blob_t *blob = malloc(sizeof(layout_blob_t));
    size_t size = sizeof(layout_blob_t);
    bool ok = blob && nvs_get_blob(h, key, blob, &size) == ESP_OK && size == sizeof(layout_blob_t) &&
              blob->version == LAYOUT_BLOB_VERSION && blob->size == sizeof(ui_layout_t) &&
              ui_layout_validate(&blob->layout, NULL, 0);
    if (ok) {
        *out = blob->layout;
    }
    free(blob);
    nvs_close(h);
    return ok;
}

static esp_err_t write_layout(uint8_t screen, const ui_layout_t *layout)
{
    ui_layout_t *stored = malloc(sizeof(ui_layout_t));
    if (stored && ui_store_load(screen, stored) && memcmp(stored, layout, sizeof(ui_layout_t)) == 0) {
        free(stored);
        return ESP_OK;
    }
    free(stored);

    layout_blob_t *blob = calloc(1, sizeof(layout_blob_t));
    if (!blob) {
        return ESP_ERR_NO_MEM;
    }
    blob->version = LAYOUT_BLOB_VERSION;
    blob->size = sizeof(ui_layout_t);
    blob->layout = *layout;

    nvs_handle_t h;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &h);
    if (err == ESP_OK) {
        char key[16];
        key_for(screen, key, sizeof(key));
        err = nvs_set_blob(h, key, blob, sizeof(layout_blob_t));
        if (err == ESP_OK) {
            err = nvs_commit(h);
            if (err == ESP_OK) {
                counters_record_nvs_write();
            }
        }
        nvs_close(h);
    }
    free(blob);
    if (err != ESP_OK) {
        ESP_LOGW(TAG, "Failed to store layout %u: %s", screen, esp_err_to_name(err));
    }
    return err;
}

static esp_err_t erase_layout(uint8_t screen)
{
    nvs_handle_t h;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &h);
    if (err != ESP_OK) {
        return err;
    }
    char key[16];
    key_for(screen, key, sizeof(key));
    err = nvs_erase_key(h, key);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        // Nothing stored: nothing to commit, no flash write to count
        nvs_close(h);
        return ESP_OK;
    }
    if (err == ESP_OK) {
        err = nvs_commit(h);
        if (err == ESP_OK) {
            counters_record_nvs_write();
        }
    }
    nvs_close(h);
    return err;
}

esp_err_t ui_store_save(uint8_t screen, const ui_layout_t *layout)
{
    if (screen >= UI_SCREEN_COUNT || !layout) {
        return ESP_ERR_INVALID_ARG;
    }
    esp_err_t err = ESP_OK;
    xSemaphoreTake(store_lock(), portMAX_DELAY);
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "Layout save blocked: state != IDLE (deferred to supervisor)");
        s_pending_layouts[screen] = *layout;
        s_dirty_save_mask |= (1 << screen);
        s_dirty_erase_mask &= ~(1 << screen);
    } else {
        // Supersedes anything still deferred for this screen
        s_dirty_save_mask &= ~(1 << screen);
        s_dirty_erase_mask &= ~(1 << screen);
        err = write_layout(screen, layout);
    }
    xSemaphoreGive(store_lock());
    return err;
}

esp_err_t ui_store_erase(uint8_t screen)
{
    if (screen >= UI_SCREEN_COUNT) {
        return ESP_ERR_INVALID_ARG;
    }
    esp_err_t err = ESP_OK;
    xSemaphoreTake(store_lock(), portMAX_DELAY);
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "Layout erase blocked: state != IDLE (deferred to supervisor)");
        s_dirty_erase_mask |= (1 << screen);
        s_dirty_save_mask &= ~(1 << screen);
    } else {
        s_dirty_save_mask &= ~(1 << screen);
        s_dirty_erase_mask &= ~(1 << screen);
        err = erase_layout(screen);
    }
    xSemaphoreGive(store_lock());
    return err;
}

esp_err_t ui_store_flush_dirty(void)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_OK;
    }
    int64_t now = esp_timer_get_time();
    if (now < s_flush_retry_at_us) {
        return ESP_OK;
    }

    esp_err_t last_err = ESP_OK;
    uint32_t flushed_count = 0;
    xSemaphoreTake(store_lock(), portMAX_DELAY);
    for (uint8_t i = 0; i < UI_SCREEN_COUNT; i++) {
        bool save = s_dirty_save_mask & (1 << i);
        bool erase = s_dirty_erase_mask & (1 << i);
        if (!save && !erase) {
            continue;
        }
        esp_err_t err = save ? write_layout(i, &s_pending_layouts[i]) : erase_layout(i);
        if (err == ESP_OK) {
            // Only once it is on flash: a failed write stays pending
            s_dirty_save_mask &= ~(1 << i);
            s_dirty_erase_mask &= ~(1 << i);
            flushed_count++;
        } else {
            last_err = err;
            diag_record(DIAG_EVENT_LAYOUT_REJECTED, 3 /* ERROR */, i, (uint32_t)err);
        }
    }
    xSemaphoreGive(store_lock());

    s_flush_retry_at_us = (last_err != ESP_OK) ? now + FLUSH_RETRY_US : 0;
    if (flushed_count > 0) {
        diag_record(DIAG_EVENT_DEFERRED_WRITE_FLUSHED, 1 /* INFO */, flushed_count, 0);
    }
    return last_err;
}
