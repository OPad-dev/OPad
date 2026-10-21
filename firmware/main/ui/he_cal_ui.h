#pragma once

/*
 * Hall Effect calibration lock screen and on-pad calibration UI.
 * docs/specs/v2-hall-effect-decisions.md D4, D7.
 *
 * Runs on Core 1 under LVGL.
 * Locks the pad when Hall Effect keys have no valid calibration:
 * - Red lock screen with download URL https://opad.dev/download and Calibrate button.
 * - If calibration-capable host app is connected, shows "Calibrate in the OPad app".
 * - On-pad interactive calibration flow: Rest -> Key 1 (5 presses) -> Key 2 (5 presses) -> Success.
 */

#include <stdbool.h>
#include <stdint.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

#define OPAD_DOWNLOAD_URL "https://opad.dev/download"

/**
 * @brief Initialize the HE calibration UI controller. Call during UI init.
 */
esp_err_t he_cal_ui_init(void);

/**
 * @brief Inform calibration UI whether a calibration-capable app is connected.
 */
void he_cal_ui_set_app_capable(bool capable);

/**
 * @brief Inform calibration UI that the host disconnected (CDC closed or unplugged).
 * Immediately clears capable-host status, cancels any app-started calibration
 * while keeping existing saved calibration untouched. After a 2 s failure
 * display, a locked pad shows the no-app screen; a calibrated pad resumes normally.
 */
void he_cal_ui_on_host_disconnect(void);

/**
 * @brief Feed touchscreen sample to the calibration UI (button hit testing).
 */
void he_cal_ui_touch_sample(bool down, int16_t x, int16_t y);

/**
 * @brief Start calibration (from button press or protocol command).
 * Refused if !he_backend_running(), outside IDLE, or already calibrating.
 * @param from_app True when requested by the host over CDC.
 * @return True if calibration started, false if refused.
 */
bool he_cal_ui_start_calibration(bool from_app);

/**
 * @brief Cancel active calibration (from protocol command), keeping old calibration.
 */
void he_cal_ui_cancel_calibration(void);

#ifdef __cplusplus
}
#endif
