#pragma once
// The "updating" animation: the MJPEG named FLASH_ANIM_ASSET in the "assets"
// partition, drawn straight to the panel without LVGL. Core 1 only; the keys
// on core 0 keep working while it plays.
#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define FLASH_ANIM_ASSET "updating"

/*
 * ROM bootloader flash (usb_cdc.c, reboot_to_rom_download): play it for up to
 * max_ms, then leave its last frame on the panel with the backlight latched
 * on while the ROM writes the flash. Takes the UI lock and never gives it
 * back: the pad restarts next. Returns at once, drawing nothing, without a
 * display or without the asset.
 */
void flash_anim_play(uint32_t max_ms);

/*
 * Update over the protocol (ota/ota_usb.c): loop it from a task of its own
 * until flash_anim_stop(). LVGL stops drawing meanwhile, and the screen does
 * not go to sleep. False, and nothing changes, without the asset.
 */
bool flash_anim_start(void);

/*
 * End flash_anim_start's loop. for_restart = false: LVGL gets the screen back
 * (the transfer failed). true: the frame on the panel stays, the backlight is
 * latched on through esp_restart(), and the next boot carries on from the
 * frame after it (flash_anim_resuming).
 */
void flash_anim_stop(bool for_restart);

/*
 * At boot, before board_display_init: whether the last run stopped for a
 * restart into a new image. Then the panel still shows that frame and must
 * not be reset. True once per such restart.
 */
bool flash_anim_resuming(void);

/*
 * After board_display_init and before LVGL has the display: play the rest of
 * the clip, from where flash_anim_stop(true) left it, for up to max_ms.
 * Blocks; LVGL's first frame then replaces the last one.
 */
void flash_anim_resume(uint32_t max_ms);

#ifdef __cplusplus
}
#endif
