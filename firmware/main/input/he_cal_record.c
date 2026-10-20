#include "he_cal_record.h"
#include <string.h>

#define ADC_RAW_MAX 4095

uint32_t he_cal_crc32(const void *data, size_t len)
{
    const uint8_t *p = (const uint8_t *)data;
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; i++) {
        crc ^= p[i];
        for (int b = 0; b < 8; b++) {
            crc = (crc >> 1) ^ (0xEDB88320u & -(crc & 1u));
        }
    }
    return ~crc;
}

void he_cal_record_init_defaults(he_cal_record_t *rec)
{
    if (!rec) {
        return;
    }
    memset(rec, 0, sizeof(*rec));
    rec->magic = HE_CAL_MAGIC;
    rec->version = HE_CAL_VERSION;
    rec->seq = 0;
    for (int k = 0; k < 2; k++) {
        rec->keys[k].rest = HE_CAL_FACTORY_REST;
        rec->keys[k].full = HE_CAL_FACTORY_FULL;
        rec->keys[k].valid = 0;
    }
    rec->crc32 = he_cal_crc32(rec, offsetof(he_cal_record_t, crc32));
}

bool he_cal_record_validate(const he_cal_record_t *rec)
{
    if (!rec) {
        return false;
    }
    if (rec->magic != HE_CAL_MAGIC) {
        return false;
    }
    if (rec->version != HE_CAL_VERSION) {
        return false;
    }

    uint32_t expected_crc = he_cal_crc32(rec, offsetof(he_cal_record_t, crc32));
    if (rec->crc32 != expected_crc) {
        return false;
    }

    for (int k = 0; k < 2; k++) {
        const he_cal_key_record_t *key = &rec->keys[k];
        if (key->valid > 1) {
            return false;
        }
        if (key->valid) {
            if (key->rest < 0 || key->rest > ADC_RAW_MAX) {
                return false;
            }
            if (key->full < 0 || key->full > ADC_RAW_MAX) {
                return false;
            }
            if (key->full <= key->rest) {
                return false;
            }
        } else {
            if (key->rest < 0 || key->rest > ADC_RAW_MAX) {
                return false;
            }
            if (key->full < 0 || key->full > ADC_RAW_MAX) {
                return false;
            }
            if (key->full <= key->rest) {
                return false;
            }
        }
    }

    return true;
}

int he_cal_record_compare_seq(uint32_t seq_a, uint32_t seq_b)
{
    if (seq_a == seq_b) {
        return 0;
    }
    return ((int32_t)(seq_a - seq_b) > 0) ? 1 : -1;
}

int he_cal_record_select_active(const he_cal_record_t *slot_a, bool a_valid,
                                const he_cal_record_t *slot_b, bool b_valid)
{
    if (a_valid && b_valid && slot_a && slot_b) {
        return (he_cal_record_compare_seq(slot_a->seq, slot_b->seq) >= 0) ? 0 : 1;
    }
    if (a_valid && slot_a) {
        return 0;
    }
    if (b_valid && slot_b) {
        return 1;
    }
    return -1;
}

int he_cal_record_next_slot(int active_slot)
{
    return (active_slot == 0) ? 1 : 0;
}

uint32_t he_cal_record_next_seq(int active_slot, uint32_t active_seq)
{
    if (active_slot < 0) {
        return 1;
    }
    uint32_t next = active_seq + 1;
    return (next == 0) ? 1 : next;
}

void he_cal_record_pack(he_cal_record_t *rec, uint32_t seq, const he_calibration_t keys[2])
{
    if (!rec) {
        return;
    }
    memset(rec, 0, sizeof(*rec));
    rec->magic = HE_CAL_MAGIC;
    rec->version = HE_CAL_VERSION;
    rec->seq = seq;
    for (int k = 0; k < 2; k++) {
        if (keys) {
            rec->keys[k].rest = keys[k].rest;
            rec->keys[k].full = keys[k].full;
            rec->keys[k].valid = keys[k].valid ? 1 : 0;
        } else {
            rec->keys[k].rest = HE_CAL_FACTORY_REST;
            rec->keys[k].full = HE_CAL_FACTORY_FULL;
            rec->keys[k].valid = 0;
        }
    }
    rec->crc32 = he_cal_crc32(rec, offsetof(he_cal_record_t, crc32));
}

void he_cal_record_unpack(const he_cal_record_t *rec, he_calibration_t keys[2])
{
    if (!rec || !keys) {
        return;
    }
    for (int k = 0; k < 2; k++) {
        keys[k].rest = rec->keys[k].rest;
        keys[k].full = rec->keys[k].full;
        keys[k].valid = rec->keys[k].valid != 0;
    }
}
