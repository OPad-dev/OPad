#include "ui_store.h"
#include "counters/counters.h"
#include "runtime/runtime.h"
#include "diag/diag.h"
#include "nvs.h"
#include "esp_log.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *TAG = "ui_store";
static const char *NVS_NAMESPACE = "osupad_ui";

static ui_layout_t s_pending_layouts[UI_SCREEN_COUNT];
static uint8_t s_dirty_save_mask = 0;
static uint8_t s_dirty_erase_mask = 0;

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

esp_err_t ui_store_save(uint8_t screen, const ui_layout_t *layout)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "Layout save blocked: state != IDLE (deferred to supervisor)");
        if (screen < UI_SCREEN_COUNT && layout) {
            s_pending_layouts[screen] = *layout;
            s_dirty_save_mask |= (1 << screen);
            s_dirty_erase_mask &= ~(1 << screen);
        }
        return ESP_OK;
    }

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

esp_err_t ui_store_erase(uint8_t screen)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        ESP_LOGW(TAG, "Layout erase blocked: state != IDLE (deferred to supervisor)");
        if (screen < UI_SCREEN_COUNT) {
            s_dirty_erase_mask |= (1 << screen);
            s_dirty_save_mask &= ~(1 << screen);
        }
        return ESP_OK;
    }

    nvs_handle_t h;
    esp_err_t err = nvs_open(NVS_NAMESPACE, NVS_READWRITE, &h);
    if (err != ESP_OK) {
        return err;
    }
    char key[16];
    key_for(screen, key, sizeof(key));
    err = nvs_erase_key(h, key);
    if (err == ESP_ERR_NVS_NOT_FOUND) {
        err = ESP_OK;
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

esp_err_t ui_store_flush_dirty(void)
{
    if (runtime_get_state() != OSUPAD_STATE_IDLE) {
        return ESP_OK;
    }

    esp_err_t last_err = ESP_OK;
    uint32_t flushed_count = 0;
    for (uint8_t i = 0; i < UI_SCREEN_COUNT; i++) {
        if (s_dirty_save_mask & (1 << i)) {
            s_dirty_save_mask &= ~(1 << i);
            esp_err_t err = ui_store_save(i, &s_pending_layouts[i]);
            if (err != ESP_OK) {
                last_err = err;
            } else {
                flushed_count++;
            }
        }
        if (s_dirty_erase_mask & (1 << i)) {
            s_dirty_erase_mask &= ~(1 << i);
            esp_err_t err = ui_store_erase(i);
            if (err != ESP_OK) {
                last_err = err;
            } else {
                flushed_count++;
            }
        }
    }
    if (flushed_count > 0) {
        diag_record(DIAG_EVENT_DEFERRED_WRITE_FLUSHED, 1 /* INFO */, flushed_count, 0);
    }
    return last_err;
}
