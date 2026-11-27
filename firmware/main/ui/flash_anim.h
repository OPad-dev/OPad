#pragma once
// The "updating" animation shown when the pad is asked into the ROM bootloader
// to be flashed (usb_cdc.c, reboot_to_rom_download).
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/*
 * Play the MJPEG named FLASH_ANIM_ASSET from the "assets" partition straight
 * to the panel for up to max_ms, then leave its last frame on the panel with
 * the backlight latched on, so the screen shows it while the ROM bootloader
 * writes the flash. Returns at once, drawing nothing, without a display or
 * without that asset (an older pack, an empty partition).
 *
 * The pad is about to restart: this takes the UI lock and never gives it
 * back, so LVGL cannot draw over the frame. Core 1 only; the keys on core 0
 * keep working while it plays.
 */
void flash_anim_play(uint32_t max_ms);

#define FLASH_ANIM_ASSET "updating"

#ifdef __cplusplus
}
#endif
