#pragma once

/*
 * Where every key backend hands over its key changes: the MX switches
 * (keypad.c) today, the Hall Effect sensors next. Only the backend for the
 * fitted module runs, so each function here has one caller at a time.
 *
 * A backend calls key_event_press_accepted() as soon as it accepts a
 * key-down (ISR-safe: counters and the press log), and key_event_report()
 * from its task for every change of the state the host should see (HID
 * report first, then latency stats and the UI wake).
 */

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    KEY_ID_1 = 0,
    KEY_ID_2 = 1,
    KEY_ID_COUNT = 2
} keypad_key_id_t;

/**
 * @brief Set up what key_event_report needs (the bench debug GPIO, if built
 * in). Call once at boot, before a backend starts.
 */
void key_event_init(void);

/**
 * @brief Count an accepted key-down: lifetime and map counters, last press
 * time, press log. ISR-safe, from IRAM. t_us is when the press happened (the
 * edge for MX).
 */
void key_event_press_accepted(uint8_t key_index, int64_t t_us);

/**
 * @brief Send a key's new state to the host (HID report), then record its
 * latency from edge_us and wake the UI on a press. Backend task only.
 */
void key_event_report(uint8_t key_index, bool pressed, int64_t edge_us);

/**
 * @brief The state last reported for a key.
 */
bool key_event_is_down(keypad_key_id_t key_id);

/**
 * @brief Get RAM lifetime press counters.
 */
void key_event_get_lifetime_presses(uint64_t *key1_presses, uint64_t *key2_presses);

/**
 * @brief Add to RAM lifetime press counters without overwriting (used during boot NVS load).
 */
void key_event_add_lifetime_presses(uint64_t key1_presses, uint64_t key2_presses);

/**
 * @brief Get press counters for the current osu! attempt.
 */
void key_event_get_map_presses(uint32_t *key1_presses, uint32_t *key2_presses);

/**
 * @brief Zero the current-attempt press counters (lifetime counters are untouched).
 */
void key_event_reset_map_presses(void);

/**
 * @brief Get timestamp in microseconds of the last keypress down event.
 */
int64_t key_event_get_last_press_us(keypad_key_id_t key_id);

#ifdef __cplusplus
}
#endif
