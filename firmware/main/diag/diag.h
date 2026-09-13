#pragma once

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#define DIAG_RING_SIZE 64

// Diagnostic Event IDs (§24.2, P2-1)
enum {
    DIAG_EVENT_NONE = 0,
    DIAG_EVENT_BOOT = 1,                 // arg0 = esp_reset_reason_t
    DIAG_EVENT_HID_MOUNTED = 2,
    DIAG_EVENT_HID_UNMOUNTED = 3,
    DIAG_EVENT_HID_SUSPENDED = 4,
    DIAG_EVENT_CDC_OPENED = 5,
    DIAG_EVENT_CDC_CLOSED = 6,
    DIAG_EVENT_NVS_INIT_FAILED = 7,      // arg0 = esp_err
    DIAG_EVENT_NVS_ERASED = 8,
    DIAG_EVENT_LCD_INIT_FAILED = 9,      // arg0 = esp_err
    DIAG_EVENT_DECODE_FAILED = 10,       // arg0 = error code
    DIAG_EVENT_FRAME_TOO_LARGE = 11,     // arg0 = frame size
    DIAG_EVENT_CONFIG_REJECTED = 12,     // arg0 = reason code
    DIAG_EVENT_COUNTER_SYNC_REJECTED = 13, // arg0 = reason code
    DIAG_EVENT_LAYOUT_REJECTED = 14,     // arg0 = screen_id
    DIAG_EVENT_DEFERRED_WRITE_FLUSHED = 15, // arg0 = count
    DIAG_EVENT_DISPLAY_SLEEP = 16,
    DIAG_EVENT_DISPLAY_WAKE = 17,
    DIAG_EVENT_LATENCY_OUTLIER = 18,     // arg0 = latency_us, arg1 = count
    DIAG_EVENT_CDC_WRITE_DROPPED = 19,   // arg0 = expected, arg1 = written
    DIAG_EVENT_BUFFER_OVERFLOW = 20,     // arg0 = dropped_count
    DIAG_EVENT_UNKNOWN_HOST_MSG = 21,    // arg0 = field tag
};

typedef struct {
    uint32_t timestamp_ms;
    uint16_t event_id;
    uint8_t level;
    uint32_t arg0;
    uint32_t arg1;
} diag_entry_t;

void diag_init(void);
void diag_record(uint16_t event_id, uint8_t level, uint32_t arg0, uint32_t arg1);
size_t diag_drain(diag_entry_t *out_entries, size_t max_entries, uint32_t *out_dropped);
size_t diag_available(void);
