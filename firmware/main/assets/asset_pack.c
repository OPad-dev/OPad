#include "assets/asset_pack.h"

#include <string.h>
#include "esp_log.h"
#include "esp_partition.h"
#include "esp_rom_crc.h"

static const char *TAG = "assets";

#define ASSET_PART_TYPE ((esp_partition_type_t)0x40)

static bool entry_ok(const asset_pack_entry_t *e, uint32_t data_start, uint32_t pack_size)
{
    if (memchr(e->name, '\0', sizeof(e->name)) == NULL) return false;
    if (e->kind != ASSET_KIND_GIF && e->kind != ASSET_KIND_MJPEG) return false;
    if (e->offset < data_start || e->offset > pack_size || e->size > pack_size - e->offset) return false;
    if (e->kind == ASSET_KIND_MJPEG) {
        // The frame table has to fit, and so do the frames it points at
        if (e->frame_count == 0 || e->frame_count > e->size / 4) return false;
    }
    return true;
}

esp_err_t asset_pack_open(asset_part_t part, asset_pack_t *out)
{
    memset(out, 0, sizeof(*out));
    if (part >= ASSET_PART_COUNT) return ESP_ERR_INVALID_ARG;

    const esp_partition_t *p =
        esp_partition_find_first(ASSET_PART_TYPE, (esp_partition_subtype_t)part, NULL);
    if (!p) {
        // A partition table from before the asset partitions
        return ESP_ERR_NOT_FOUND;
    }

    // The header first, to learn how much to map
    const void *ptr = NULL;
    esp_partition_mmap_handle_t handle;
    esp_err_t err = esp_partition_mmap(p, 0, sizeof(asset_pack_header_t), ESP_PARTITION_MMAP_DATA, &ptr, &handle);
    if (err != ESP_OK) return err;
    asset_pack_header_t h;
    memcpy(&h, ptr, sizeof(h));
    esp_partition_munmap(handle);

    if (h.magic != ASSET_PACK_MAGIC) return ESP_ERR_NOT_FOUND;  // erased, or never written
    if (h.format != ASSET_PACK_FORMAT || h.header_size < sizeof(asset_pack_header_t) ||
        h.entry_size < sizeof(asset_pack_entry_t)) {
        ESP_LOGW(TAG, "%s: pack format %u is not one this firmware reads", p->label, h.format);
        return ESP_ERR_INVALID_VERSION;
    }
    uint32_t table_end = h.header_size + (uint32_t)h.entry_count * h.entry_size;
    if (h.pack_size > p->size || table_end > h.pack_size) {
        ESP_LOGW(TAG, "%s: pack of %lu bytes does not fit", p->label, (unsigned long)h.pack_size);
        return ESP_ERR_INVALID_SIZE;
    }

    err = esp_partition_mmap(p, 0, h.pack_size, ESP_PARTITION_MMAP_DATA, &ptr, &handle);
    if (err != ESP_OK) {
        ESP_LOGE(TAG, "%s: mapping %lu bytes failed: %s", p->label, (unsigned long)h.pack_size,
                 esp_err_to_name(err));
        return err;
    }
    const uint8_t *base = ptr;
    // Entries are read through this pointer, so the stride has to be ours
    if (h.entry_size != sizeof(asset_pack_entry_t)) {
        esp_partition_munmap(handle);
        return ESP_ERR_INVALID_VERSION;
    }
    const asset_pack_entry_t *entries = (const asset_pack_entry_t *)(base + h.header_size);
    for (uint16_t i = 0; i < h.entry_count; i++) {
        if (!entry_ok(&entries[i], table_end, h.pack_size)) {
            ESP_LOGW(TAG, "%s: entry %u is malformed", p->label, i);
            esp_partition_munmap(handle);
            return ESP_ERR_INVALID_SIZE;
        }
    }

    out->base = base;
    out->header = (const asset_pack_header_t *)base;
    out->entries = entries;
    out->entry_count = h.entry_count;
    out->partition_size = p->size;
    ESP_LOGI(TAG, "%s: pack \"%.16s\", %u entries, %lu bytes", p->label, h.label, h.entry_count,
             (unsigned long)h.pack_size);
    return ESP_OK;
}

esp_err_t asset_pack_verify(const asset_pack_t *pack)
{
    if (!pack->base) return ESP_ERR_INVALID_STATE;
    const asset_pack_header_t *h = pack->header;
    uint32_t crc = esp_rom_crc32_le(0, pack->base + h->header_size, h->pack_size - h->header_size);
    return crc == h->crc32 ? ESP_OK : ESP_ERR_INVALID_CRC;
}

const uint8_t *asset_pack_data(const asset_pack_t *pack, uint16_t i)
{
    if (!pack->base || i >= pack->entry_count) return NULL;
    return pack->base + pack->entries[i].offset;
}
