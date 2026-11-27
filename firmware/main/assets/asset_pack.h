#pragma once
// Asset packs: the screens and animations in the "assets" (OPad's) and "user"
// (the owner's) flash partitions, firmware/partitions.csv.
//
// A pack is read in place through the flash cache (esp_partition_mmap), never
// with esp_flash_read, which turns the cache off on both cores and would stall
// the keys on core 0. scripts/build_asset_pack.py writes packs; this layout and
// that script change together.
//
// Layout, little-endian:
//   asset_pack_header_t   at 0
//   asset_pack_entry_t    entry_count of them, right after the header
//   entry data            each at its entry's offset (from the pack start)
// header.crc32 covers every byte after the header up to pack_size, so a pack
// cut short by an interrupted write reads as absent, never as half a pack.
//
// Entry data by kind:
//   ASSET_KIND_GIF    a GIF file as is
//   ASSET_KIND_MJPEG  uint32_t ends[frame_count] (frame i is the bytes from
//                     ends[i-1], or the end of the table for frame 0, up to
//                     ends[i], relative to the entry's data) then the
//                     baseline JPEG frames back to back
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

#define ASSET_PACK_MAGIC 0x4B41504Fu  // "OPAK"
#define ASSET_PACK_FORMAT 1
#define ASSET_PACK_NAME_LEN 32

typedef enum {
    ASSET_KIND_GIF = 1,
    ASSET_KIND_MJPEG = 2,
} asset_kind_t;

typedef struct __attribute__((packed)) {
    uint32_t magic;
    uint16_t format;
    uint16_t header_size;   // sizeof(asset_pack_header_t) when written
    uint16_t entry_count;
    uint16_t entry_size;    // sizeof(asset_pack_entry_t) when written
    uint32_t pack_size;     // header, table and data
    uint32_t crc32;         // esp_rom_crc32_le(0, ...) of bytes header_size..pack_size
    char label[16];
    uint8_t reserved[28];
} asset_pack_header_t;

typedef struct __attribute__((packed)) {
    char name[ASSET_PACK_NAME_LEN];  // NUL-terminated
    uint8_t kind;                    // asset_kind_t
    uint8_t reserved0[3];
    uint16_t width;
    uint16_t height;
    uint32_t frame_count;
    uint32_t frame_us;               // MJPEG: time per frame; GIF: 0 (the file has its own)
    uint32_t offset;                 // from the pack start
    uint32_t size;
    uint8_t reserved[8];
} asset_pack_entry_t;

_Static_assert(sizeof(asset_pack_header_t) == 64, "asset pack header is 64 bytes");
_Static_assert(sizeof(asset_pack_entry_t) == 64, "asset pack entry is 64 bytes");

typedef enum {
    ASSET_PART_OPAD = 0,  // "assets", subtype 0
    ASSET_PART_USER = 1,  // "user", subtype 1
    ASSET_PART_COUNT
} asset_part_t;

typedef struct {
    const uint8_t *base;  // mapped pack, NULL when absent
    const asset_pack_header_t *header;
    const asset_pack_entry_t *entries;
    uint16_t entry_count;
    uint32_t partition_size;
} asset_pack_t;

/*
 * Map the pack in `part` and check its header and table (not the CRC: see
 * asset_pack_verify). ESP_ERR_NOT_FOUND when the partition or a pack is not
 * there, ESP_ERR_INVALID_SIZE / ESP_ERR_INVALID_VERSION when it is not one this
 * firmware reads. The mapping stays for the life of the firmware. Not on the
 * keypress path; call from core 1.
 */
esp_err_t asset_pack_open(asset_part_t part, asset_pack_t *out);

// CRC of the whole pack against its header: reads every byte, so it takes a
// while on a full partition. ESP_ERR_INVALID_CRC when they differ.
esp_err_t asset_pack_verify(const asset_pack_t *pack);

// The data of entry i, or NULL when i is out of range
const uint8_t *asset_pack_data(const asset_pack_t *pack, uint16_t i);

#ifdef __cplusplus
}
#endif
