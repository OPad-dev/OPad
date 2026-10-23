#pragma once

/*
 * MX debug log, the MX half of the debug firmware (CONFIG_OSUPAD_HE_DEBUG):
 * for chasing taps an MX pad never sent. Tagged "he.mx*" so opad-daemon
 * writes the lines to he-debug.log as it does the HE ones:
 *
 *   he.mxtap   every tap the pad made: how long it was held, the bounces
 *              debouncing ignored, and whether a resample made the edge
 *   he.mxmiss  a pin read pressed for a while with no press reported: MISS
 *              (released again with no press at all), LATE (the press came,
 *              late) or STUCK (reported down, pin up)
 *   he.mxmod   keys turned off or back on by the module monitor, with the ID
 *              voltage that did it
 *   he.mxsec   per second with anything to say: presses, ignored edges and the
 *              ID line's range
 *
 * It costs latency (a 1 kHz pin sampler, a few stores in the key ISR), which
 * is accepted for a debug build. Outside it every call below is a no-op.
 */

#include <stdbool.h>
#include <stdint.h>
#include "sdkconfig.h"

#ifdef __cplusplus
extern "C" {
#endif

#ifdef CONFIG_OSUPAD_HE_DEBUG

// Key ISR or keypad task, under the keypad spinlock: debouncing accepted a
// transition (`resample`: at the end of a lockout, not on an edge)
void mx_debug_transition(uint8_t key, bool pressed, int64_t now_us, bool resample);
// Key ISR, under the keypad spinlock: an edge debouncing ignored
void mx_debug_suppressed(uint8_t key);

// Module monitor task only
void mx_debug_module(bool keys_on, int id_mv);
void mx_debug_id_mv(int id_mv);

void mx_debug_start(void);

#else

static inline void mx_debug_transition(uint8_t key, bool pressed, int64_t now_us, bool resample)
{
    (void)key; (void)pressed; (void)now_us; (void)resample;
}
static inline void mx_debug_suppressed(uint8_t key) { (void)key; }
static inline void mx_debug_module(bool keys_on, int id_mv) { (void)keys_on; (void)id_mv; }
static inline void mx_debug_id_mv(int id_mv) { (void)id_mv; }
static inline void mx_debug_start(void) {}

#endif

#ifdef __cplusplus
}
#endif
