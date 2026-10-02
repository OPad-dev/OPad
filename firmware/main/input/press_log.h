#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef ESP_PLATFORM
#include "esp_attr.h"
#else
#ifndef IRAM_ATTR
#define IRAM_ATTR
#endif
#endif

#ifdef __cplusplus
extern "C" {
#endif

// Accepted key-downs with their pad timestamps, for the host's tap rate (issue #2).
//
// One producer, the input path (key ISR and keypad task, serialised by the keypad
// spinlock), and one consumer, the protocol task. Lock-free: pushing is one store
// and one atomic increment, so recording a press adds nothing measurable to the
// input path. A full log drops the newest press and counts it.

#define PRESS_LOG_CAPACITY 128  // power of two

typedef struct {
    int64_t t_us;   // esp_timer time of the accepted press
    uint8_t key;    // 1 = K1, 2 = K2
} press_log_entry_t;

/** Record an accepted key-down. Input path only. */
void press_log_push(uint8_t key, int64_t t_us);

/** Move up to max presses, oldest first, into out; adds the presses dropped since
 *  the last call to *dropped. Returns how many were moved. Protocol task only. */
size_t press_log_drain(press_log_entry_t *out, size_t max, uint32_t *dropped);

/** Presses waiting to be drained. */
size_t press_log_pending(void);

/** Forget everything waiting (no map is being played). Protocol task only. */
void press_log_clear(void);

#ifdef __cplusplus
}
#endif
