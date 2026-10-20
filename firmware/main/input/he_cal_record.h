#pragma once

/*
 * Pure-C Hall Effect calibration record and A/B slot integrity check.
 * docs/specs/v2-hall-effect-decisions.md D7.
 *
 * Explicit per-key calibration: rest and normal full press reading.
 * Stored as A/B records with sequence numbers and CRC32 in NVS. Missing or
 * corrupt records fall back to factory defaults (valid=false) without
 * erasing anything.
 *
 * Runs off the key path; pure C, no FreeRTOS/ESP-IDF dependencies, host-tested.
 */

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "input/he_params.h"

#ifdef __cplusplus
extern "C" {
#endif

#define HE_CAL_MAGIC        0x4F504341u  // "OPCA" (OPad CAlibration)
#define HE_CAL_VERSION      1u

// Factory defaults (provisional bench values, docs/specs/v2-hall-effect-decisions.md D7)
#define HE_CAL_FACTORY_REST 2086
#define HE_CAL_FACTORY_FULL (2086 + 450)

typedef struct {
    int32_t rest;
    int32_t full;
    uint8_t valid;
    uint8_t rsv[3];
} he_cal_key_record_t;

typedef struct {
    uint32_t magic;
    uint16_t version;
    uint16_t reserved;
    uint32_t seq;
    he_cal_key_record_t keys[2];
    uint32_t crc32;
} he_cal_record_t;

/**
 * Standard CRC-32 (IEEE 802.3, polynomial 0xEDB88320).
 */
uint32_t he_cal_crc32(const void *data, size_t len);

/**
 * Fill record with factory defaults (valid = false).
 */
void he_cal_record_init_defaults(he_cal_record_t *rec);

/**
 * Check magic, version, CRC, and physical value ranges.
 * Returns true if valid, false if corrupt.
 */
bool he_cal_record_validate(const he_cal_record_t *rec);

/**
 * Compare two sequence numbers with unsigned 32-bit wrap-around.
 * Returns >0 if seq_a is newer, <0 if seq_b is newer, 0 if equal.
 */
int he_cal_record_compare_seq(uint32_t seq_a, uint32_t seq_b);

/**
 * Select the active slot between A and B based on validity and sequence number.
 * Returns:
 *   0 if slot A is active,
 *   1 if slot B is active,
 *  -1 if neither slot is valid.
 */
int he_cal_record_select_active(const he_cal_record_t *slot_a, bool a_valid,
                                const he_cal_record_t *slot_b, bool b_valid);

/**
 * Determine which slot to write next (alternates A/B).
 */
int he_cal_record_next_slot(int active_slot);

/**
 * Determine the sequence number for the next write.
 */
uint32_t he_cal_record_next_seq(int active_slot, uint32_t active_seq);

/**
 * Pack calibration values and sequence number into a record and calculate CRC.
 */
void he_cal_record_pack(he_cal_record_t *rec, uint32_t seq, const he_calibration_t keys[2]);

/**
 * Unpack a validated record into he_calibration_t array.
 */
void he_cal_record_unpack(const he_cal_record_t *rec, he_calibration_t keys[2]);

#ifdef __cplusplus
}
#endif
