#pragma once

#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * @brief Initialize USB CDC-ACM communication subsystem.
 */
esp_err_t usb_cdc_init(void);

/**
 * @brief Check if host is connected over CDC (DTR line asserted).
 */
bool usb_cdc_is_connected(void);

/**
 * @brief Write bytes to the USB CDC interface.
 * @param data Data buffer to transmit
 * @param len Length in bytes
 * @return Number of bytes successfully queued/written
 */
size_t usb_cdc_write(const uint8_t *data, size_t len);

/**
 * @brief Flush CDC write buffer.
 */
void usb_cdc_flush(void);

/**
 * @brief Start the core-1 task that handles incoming CDC data and the host protocol.
 */
esp_err_t usb_cdc_start_task(void);

/**
 * @brief Reboot chip directly into the ROM download bootloader.
 */
void usb_cdc_reboot_to_bootloader(void);

#ifdef __cplusplus
}
#endif
