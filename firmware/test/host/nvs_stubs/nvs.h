#pragma once
// The slice of ESP-IDF's NVS API config/nvs_ns.c uses, backed by the fake in test_nvs_ns.c
#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"
typedef int nvs_handle_t;
#define NVS_READONLY 0
#define NVS_READWRITE 1
#define NVS_DEFAULT_PART_NAME "nvs"
typedef enum {
    NVS_TYPE_U8 = 0x01, NVS_TYPE_I8 = 0x11, NVS_TYPE_U16 = 0x02, NVS_TYPE_I16 = 0x12,
    NVS_TYPE_U32 = 0x04, NVS_TYPE_I32 = 0x14, NVS_TYPE_U64 = 0x08, NVS_TYPE_I64 = 0x18,
    NVS_TYPE_STR = 0x21, NVS_TYPE_BLOB = 0x42, NVS_TYPE_ANY = 0xff
} nvs_type_t;
typedef struct {
    char namespace_name[16];
    char key[16];
    nvs_type_t type;
} nvs_entry_info_t;
typedef struct nvs_fake_iter *nvs_iterator_t;
esp_err_t nvs_open(const char *name, int mode, nvs_handle_t *handle);
void nvs_close(nvs_handle_t handle);
esp_err_t nvs_commit(nvs_handle_t handle);
esp_err_t nvs_erase_all(nvs_handle_t handle);
esp_err_t nvs_entry_find(const char *part, const char *ns, nvs_type_t type, nvs_iterator_t *it);
esp_err_t nvs_entry_next(nvs_iterator_t *it);
esp_err_t nvs_entry_info(nvs_iterator_t it, nvs_entry_info_t *out);
void nvs_release_iterator(nvs_iterator_t it);
#define NVS_INT(T, N)                                                  \
    esp_err_t nvs_get_##N(nvs_handle_t h, const char *key, T *out);    \
    esp_err_t nvs_set_##N(nvs_handle_t h, const char *key, T value);
NVS_INT(uint8_t, u8) NVS_INT(int8_t, i8) NVS_INT(uint16_t, u16) NVS_INT(int16_t, i16)
NVS_INT(uint32_t, u32) NVS_INT(int32_t, i32) NVS_INT(uint64_t, u64) NVS_INT(int64_t, i64)
#undef NVS_INT
esp_err_t nvs_get_str(nvs_handle_t h, const char *key, char *out, size_t *len);
esp_err_t nvs_set_str(nvs_handle_t h, const char *key, const char *value);
esp_err_t nvs_get_blob(nvs_handle_t h, const char *key, void *out, size_t *len);
esp_err_t nvs_set_blob(nvs_handle_t h, const char *key, const void *value, size_t len);
