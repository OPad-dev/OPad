#pragma once

#include <stdint.h>
#include "esp_err.h"

// Calibration-only, serialized caller, never the TinyUSB task itself. Waits
// for any running USB callback to return. Existing transfers may still finish.
// timeout_ms bounds the acknowledgement wait after TinyUSB queues the fence;
// its task-context enqueue may wait for queue room (no LVGL-dependent callback).
esp_err_t usb_report_barrier_wait(uint32_t timeout_ms);
