// The osupad* -> opad* NVS namespace copy (config/nvs_ns.c), against an
// in-memory NVS: full copy, a fresh pad, a second boot, a copy cut short by a
// reset, and an NVS too full for the copy.
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "config/nvs_ns.h"
#include "nvs.h"

#define MAX_ENTRIES 64
#define MAX_DATA 4096

typedef struct {
    bool used;
    char ns[16];
    char key[16];
    nvs_type_t type;
    uint8_t data[MAX_DATA];
    size_t len;
} entry_t;

static entry_t store[MAX_ENTRIES];
static char namespaces[16][16];
static int ns_count;
static int writes_left = -1;  // -1: unlimited; 0: the next write fails as NVS full
static char open_ns[16][16];  // handle -> namespace

const char *esp_err_to_name(esp_err_t code) { return code == ESP_OK ? "ESP_OK" : "error"; }

static bool ns_exists(const char *ns)
{
    for (int i = 0; i < ns_count; i++) {
        if (strcmp(namespaces[i], ns) == 0) return true;
    }
    return false;
}

static void reset_store(void)
{
    memset(store, 0, sizeof(store));
    ns_count = 0;
    writes_left = -1;
}

esp_err_t nvs_open(const char *name, int mode, nvs_handle_t *handle)
{
    if (!ns_exists(name)) {
        if (mode == NVS_READONLY) return ESP_ERR_NVS_NOT_FOUND;
        strcpy(namespaces[ns_count++], name);
    }
    for (int h = 1; h < 16; h++) {
        if (open_ns[h][0] == 0) {
            strcpy(open_ns[h], name);
            *handle = h;
            return ESP_OK;
        }
    }
    return ESP_FAIL;
}

void nvs_close(nvs_handle_t h) { open_ns[h][0] = 0; }
esp_err_t nvs_commit(nvs_handle_t h) { (void)h; return ESP_OK; }

static entry_t *find(const char *ns, const char *key)
{
    for (int i = 0; i < MAX_ENTRIES; i++) {
        if (store[i].used && strcmp(store[i].ns, ns) == 0 && strcmp(store[i].key, key) == 0) return &store[i];
    }
    return NULL;
}

static esp_err_t put(nvs_handle_t h, const char *key, nvs_type_t type, const void *data, size_t len)
{
    assert(len <= MAX_DATA);
    if (writes_left == 0) return ESP_ERR_NVS_NOT_ENOUGH_SPACE;
    if (writes_left > 0) writes_left--;
    entry_t *e = find(open_ns[h], key);
    for (int i = 0; !e && i < MAX_ENTRIES; i++) {
        if (!store[i].used) e = &store[i];
    }
    assert(e);
    e->used = true;
    strcpy(e->ns, open_ns[h]);
    strcpy(e->key, key);
    e->type = type;
    memcpy(e->data, data, len);
    e->len = len;
    return ESP_OK;
}

static esp_err_t get(nvs_handle_t h, const char *key, nvs_type_t type, void *out, size_t *len)
{
    entry_t *e = find(open_ns[h], key);
    if (!e || e->type != type) return ESP_ERR_NVS_NOT_FOUND;
    if (out) {
        if (*len < e->len) return ESP_FAIL;
        memcpy(out, e->data, e->len);
    }
    *len = e->len;
    return ESP_OK;
}

esp_err_t nvs_erase_all(nvs_handle_t h)
{
    for (int i = 0; i < MAX_ENTRIES; i++) {
        if (store[i].used && strcmp(store[i].ns, open_ns[h]) == 0) store[i].used = false;
    }
    return ESP_OK;
}

#define NVS_INT(T, N, TYPE)                                                        \
    esp_err_t nvs_get_##N(nvs_handle_t h, const char *key, T *out)                 \
    {                                                                              \
        size_t len = sizeof(T);                                                    \
        return get(h, key, TYPE, out, &len);                                       \
    }                                                                              \
    esp_err_t nvs_set_##N(nvs_handle_t h, const char *key, T value)                \
    {                                                                              \
        return put(h, key, TYPE, &value, sizeof(T));                               \
    }
NVS_INT(uint8_t, u8, NVS_TYPE_U8)
NVS_INT(int8_t, i8, NVS_TYPE_I8)
NVS_INT(uint16_t, u16, NVS_TYPE_U16)
NVS_INT(int16_t, i16, NVS_TYPE_I16)
NVS_INT(uint32_t, u32, NVS_TYPE_U32)
NVS_INT(int32_t, i32, NVS_TYPE_I32)
NVS_INT(uint64_t, u64, NVS_TYPE_U64)
NVS_INT(int64_t, i64, NVS_TYPE_I64)
#undef NVS_INT

esp_err_t nvs_get_str(nvs_handle_t h, const char *key, char *out, size_t *len) { return get(h, key, NVS_TYPE_STR, out, len); }
esp_err_t nvs_set_str(nvs_handle_t h, const char *key, const char *v) { return put(h, key, NVS_TYPE_STR, v, strlen(v) + 1); }
esp_err_t nvs_get_blob(nvs_handle_t h, const char *key, void *out, size_t *len) { return get(h, key, NVS_TYPE_BLOB, out, len); }
esp_err_t nvs_set_blob(nvs_handle_t h, const char *key, const void *v, size_t len) { return put(h, key, NVS_TYPE_BLOB, v, len); }

struct nvs_fake_iter {
    char ns[16];
    int pos;
};

static esp_err_t advance(nvs_iterator_t it, int from)
{
    for (int i = from; i < MAX_ENTRIES; i++) {
        if (store[i].used && strcmp(store[i].ns, it->ns) == 0) {
            it->pos = i;
            return ESP_OK;
        }
    }
    return ESP_ERR_NVS_NOT_FOUND;
}

esp_err_t nvs_entry_find(const char *part, const char *ns, nvs_type_t type, nvs_iterator_t *out)
{
    (void)part;
    (void)type;
    nvs_iterator_t it = calloc(1, sizeof(*it));
    strcpy(it->ns, ns);
    if (advance(it, 0) != ESP_OK) {
        free(it);
        *out = NULL;
        return ESP_ERR_NVS_NOT_FOUND;
    }
    *out = it;
    return ESP_OK;
}

esp_err_t nvs_entry_next(nvs_iterator_t *it)
{
    if (advance(*it, (*it)->pos + 1) != ESP_OK) {
        free(*it);
        *it = NULL;
        return ESP_ERR_NVS_NOT_FOUND;
    }
    return ESP_OK;
}

esp_err_t nvs_entry_info(nvs_iterator_t it, nvs_entry_info_t *out)
{
    strcpy(out->namespace_name, it->ns);
    strcpy(out->key, store[it->pos].key);
    out->type = store[it->pos].type;
    return ESP_OK;
}

void nvs_release_iterator(nvs_iterator_t it) { free(it); }

// --- helpers -----------------------------------------------------------------

static void seed(const char *ns, const char *key, nvs_type_t type, const void *data, size_t len)
{
    nvs_handle_t h;
    assert(nvs_open(ns, NVS_READWRITE, &h) == ESP_OK);
    assert(put(h, key, type, data, len) == ESP_OK);
    nvs_close(h);
}

static int count_in(const char *ns)
{
    int n = 0;
    for (int i = 0; i < MAX_ENTRIES; i++) n += store[i].used && strcmp(store[i].ns, ns) == 0;
    return n;
}

static bool same(const char *ns_a, const char *ns_b, const char *key)
{
    entry_t *a = find(ns_a, key), *b = find(ns_b, key);
    return a && b && a->type == b->type && a->len == b->len && memcmp(a->data, b->data, a->len) == 0;
}

static uint8_t layout[2312];

// A pad as 1.0.0-rc left it: counters, a config blob, a calibration pair, a layout
static void seed_legacy_pad(void)
{
    uint32_t gen = 15;
    uint64_t k1 = 3745, k2 = 20594;
    seed("osupad", "generation", NVS_TYPE_U32, &gen, sizeof(gen));
    seed("osupad", "key1_cnt", NVS_TYPE_U64, &k1, sizeof(k1));
    seed("osupad", "key2_cnt", NVS_TYPE_U64, &k2, sizeof(k2));
    uint8_t cfg[96];
    for (size_t i = 0; i < sizeof(cfg); i++) cfg[i] = (uint8_t)(i * 7);
    seed("osupad_cfg", "config", NVS_TYPE_BLOB, cfg, sizeof(cfg));
    seed("osupad_cal3", "cal3_a", NVS_TYPE_BLOB, cfg, 32);
    seed("osupad_cal3", "cal3_b", NVS_TYPE_BLOB, cfg + 1, 32);
    for (size_t i = 0; i < sizeof(layout); i++) layout[i] = (uint8_t)(i ^ 0x5a);
    seed("osupad_ui", "layout0", NVS_TYPE_BLOB, layout, sizeof(layout));
    seed("osupad_ui", "name", NVS_TYPE_STR, "test", 5);
}

static void test_full_copy(void)
{
    reset_store();
    seed_legacy_pad();
    int legacy_entries = count_in("osupad") + count_in("osupad_cfg") + count_in("osupad_cal3") + count_in("osupad_ui");
    nvs_ns_migrate();
    for (int i = 0; i < NVS_NS_COUNT; i++) {
        assert(strncmp(nvs_ns_name(i), "opad", 4) == 0);
    }
    assert(same("osupad", "opad", "generation"));
    assert(same("osupad", "opad", "key1_cnt"));
    assert(same("osupad", "opad", "key2_cnt"));
    assert(same("osupad_cfg", "opad_cfg", "config"));
    assert(same("osupad_cal3", "opad_cal3", "cal3_a"));
    assert(same("osupad_cal3", "opad_cal3", "cal3_b"));
    assert(same("osupad_ui", "opad_ui", "layout0"));
    assert(same("osupad_ui", "opad_ui", "name"));
    // Each new namespace holds the copy plus its marker; the old ones are untouched
    assert(count_in("opad") == 4 && count_in("opad_cfg") == 2 && count_in("opad_cal3") == 3);
    assert(count_in("opad_ui") == 3 && count_in("opad_cal") == 1);
    assert(count_in("osupad") + count_in("osupad_cfg") + count_in("osupad_cal3") + count_in("osupad_ui") ==
           legacy_entries);
    printf("  ✓ test_full_copy passed\n");
}

static void test_fresh_pad(void)
{
    reset_store();
    nvs_ns_migrate();
    for (int i = 0; i < NVS_NS_COUNT; i++) {
        assert(strncmp(nvs_ns_name(i), "opad", 4) == 0);
        assert(count_in(nvs_ns_name(i)) == 1);  // the marker alone
        assert(!ns_exists(nvs_ns_legacy_name(i)));
    }
    printf("  ✓ test_fresh_pad passed\n");
}

static void test_second_boot_keeps_new_data(void)
{
    reset_store();
    seed_legacy_pad();
    nvs_ns_migrate();
    uint64_t newer = 9999;
    seed("opad", "key1_cnt", NVS_TYPE_U64, &newer, sizeof(newer));
    nvs_ns_migrate();
    uint64_t got = 0;
    nvs_handle_t h;
    nvs_open("opad", NVS_READONLY, &h);
    assert(nvs_get_u64(h, "key1_cnt", &got) == ESP_OK && got == 9999);
    nvs_close(h);
    printf("  ✓ test_second_boot_keeps_new_data passed\n");
}

static void test_copy_cut_short_by_a_reset(void)
{
    reset_store();
    seed_legacy_pad();
    // What a reset mid-copy leaves: some entries, no marker
    uint64_t partial = 1;
    seed("opad", "key1_cnt", NVS_TYPE_U64, &partial, sizeof(partial));
    seed("opad", "stray", NVS_TYPE_U8, &partial, 1);
    nvs_ns_migrate();
    assert(same("osupad", "opad", "key1_cnt"));
    assert(!find("opad", "stray"));
    assert(strcmp(nvs_ns_name(NVS_NS_COUNTERS), "opad") == 0);
    printf("  ✓ test_copy_cut_short_by_a_reset passed\n");
}

static void test_nvs_full_keeps_old_names(void)
{
    reset_store();
    seed_legacy_pad();
    writes_left = 2;  // the first namespace's copy runs out of space
    nvs_ns_migrate();
    assert(strcmp(nvs_ns_name(NVS_NS_COUNTERS), "osupad") == 0);
    assert(count_in("opad") == 0);  // the partial copy was erased
    assert(find("osupad", "key1_cnt"));
    // Space again at the next boot: the copy completes
    writes_left = -1;
    nvs_ns_migrate();
    assert(strcmp(nvs_ns_name(NVS_NS_COUNTERS), "opad") == 0);
    assert(same("osupad", "opad", "key2_cnt"));
    printf("  ✓ test_nvs_full_keeps_old_names passed\n");
}

int main(void)
{
    printf("Running NVS namespace migration tests...\n");
    test_full_copy();
    test_fresh_pad();
    test_second_boot_keeps_new_data();
    test_copy_cut_short_by_a_reset();
    test_nvs_full_keeps_old_names();
    printf("All NVS namespace migration tests passed!\n");
    return 0;
}
