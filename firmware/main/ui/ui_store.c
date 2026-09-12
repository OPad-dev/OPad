#include "ui_store.h"
#include "nvs.h"
#include "esp_log.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char *TAG = "ui_store";
static const char *NVS_NAMESPACE = "osupad_ui";

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
    }
    nvs_close(h);
    return err;
}
