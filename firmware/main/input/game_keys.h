#pragma once

/*
 * Game keys: the keys a player plays with (device config game_keys, bit k =
 * key k+1). Totals and the tap rate count only them, and the screen's two
 * key cards (pad.k1_*, pad.k2_*) show the first and second of them, so a
 * player on keys 1 and 3 sees key 3 where key 2 would be. With the default,
 * keys 1 and 2, every card shows what it always did. Pure C, host-tested
 * (firmware/test/host/test_game_keys.c).
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define GAME_KEYS_MAX 3

/**
 * @brief The game keys in order: out[slot] = key index (0-2), -1 for a slot
 * with no key (one game key only). A third game key gets no card slot; it
 * still counts in the totals.
 */
static inline void game_keys_slots(uint8_t mask, int out[2])
{
    int n = 0;
    out[0] = -1;
    out[1] = -1;
    for (int k = 0; k < GAME_KEYS_MAX && n < 2; k++) {
        if (mask & (1u << k)) {
            out[n++] = k;
        }
    }
}

/** @brief Sum of v[] over the game keys. */
static inline uint64_t game_keys_sum(uint8_t mask, const uint64_t v[GAME_KEYS_MAX])
{
    uint64_t sum = 0;
    for (int k = 0; k < GAME_KEYS_MAX; k++) {
        if (mask & (1u << k)) {
            sum += v[k];
        }
    }
    return sum;
}

/**
 * @brief A key card's label for a HID usage: the letter or digit it types,
 * else "" (the layout's own text stays). out holds at least 2 chars.
 */
static inline void game_keys_label(uint8_t usage, char out[2])
{
    out[1] = '\0';
    if (usage >= 0x04 && usage <= 0x1D) {
        out[0] = (char)('A' + (usage - 0x04));
    } else if (usage >= 0x1E && usage <= 0x26) {
        out[0] = (char)('1' + (usage - 0x1E));
    } else if (usage == 0x27) {
        out[0] = '0';
    } else {
        out[0] = '\0';
    }
}

#ifdef __cplusplus
}
#endif
