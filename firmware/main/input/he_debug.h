#pragma once

/*
 * Hall Effect debug log (CONFIG_OSUPAD_HE_DEBUG). A debug firmware, not a
 * product build: it logs every press and release the engine makes, with the
 * readings just before it, plus per-second reading ranges, ADC health and
 * context (pad state, screen animation, chip temperature). Lines go to the
 * host as LogEvents (tags "he.*"), so it works during a map with opad-daemon
 * and tosu running; the daemon also writes them to he-debug.log.
 *
 * It costs latency (work in the frame interrupt, a busier CDC task), which
 * is accepted for a debug build. Outside it every call below is a no-op.
 */

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "sdkconfig.h"
#include "input/he_rt.h"

#ifdef __cplusplus
extern "C" {
#endif

// Frames of history logged before each press or release (250 us each)
#define HE_DEBUG_PRE 12

typedef struct {
    char tag[16];
    char msg[64];
    uint32_t ms;
    uint8_t level;  // 0 debug, 1 info, 2 warn, 3 error (LogLevel)
} he_debug_line_t;

#ifdef CONFIG_OSUPAD_HE_DEBUG

// Frame interrupt, under the backend's s_lock: newest reading per key (-1 = none)
void he_debug_isr_frame(int32_t raw0, int32_t raw1);
// Frame interrupt: the engine just pressed or released key `key`
void he_debug_isr_edge(uint8_t key, he_rt_event_t ev, he_rt_state_t from, he_rt_state_t to,
                       int32_t extreme, int32_t raw, int64_t now_us);

// Task context only (never the interrupt)
void he_debug_start(void);
// The line ring both halves log into (HE here, MX in mx_debug.c); true once open
bool he_debug_log_open(void);
void he_debug_line(const char *tag, uint8_t level, const char *fmt, ...)
    __attribute__((format(printf, 3, 4)));
// Stamped `ms` on the pad's clock (when it happened, not when it was formatted)
void he_debug_line_at(const char *tag, uint8_t level, uint32_t ms, const char *fmt, ...)
    __attribute__((format(printf, 4, 5)));
void he_debug_params(uint8_t key, const he_rt_params_t *p);
size_t he_debug_take(he_debug_line_t *out, size_t max);
bool he_debug_pending(void);

#else

static inline void he_debug_isr_frame(int32_t raw0, int32_t raw1) { (void)raw0; (void)raw1; }
static inline void he_debug_isr_edge(uint8_t key, he_rt_event_t ev, he_rt_state_t from, he_rt_state_t to,
                                     int32_t extreme, int32_t raw, int64_t now_us)
{
    (void)key; (void)ev; (void)from; (void)to; (void)extreme; (void)raw; (void)now_us;
}
static inline void he_debug_start(void) {}
static inline void he_debug_params(uint8_t key, const he_rt_params_t *p) { (void)key; (void)p; }
static inline bool he_debug_pending(void) { return false; }

#endif

#ifdef __cplusplus
}
#endif
