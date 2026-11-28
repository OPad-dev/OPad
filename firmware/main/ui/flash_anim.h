#pragma once
// The "updating" and "boot" animations: the MJPEGs named FLASH_ANIM_ASSET
// and BOOT_ANIM_ASSET in the "assets" partition, drawn straight to the panel
// without LVGL. Core 1 only; the keys on core 0 keep working while they play.
#include <stdbool.h>
#include <stdint.h>
#include "lvgl.h"

#ifdef __cplusplus
extern "C" {
#endif

#define FLASH_ANIM_ASSET "updating"
#define BOOT_ANIM_ASSET "boot"

/*
 * ROM bootloader flash (usb_cdc.c, reboot_to_rom_download): play it for up to
 * max_ms, then leave its last frame on the panel with the backlight latched
 * on while the ROM writes the flash. Takes the UI lock and never gives it
 * back: the pad restarts next. Returns at once, drawing nothing, without a
 * display or without the asset.
 */
void flash_anim_play(uint32_t max_ms);

/*
 * Update over the protocol (ota/ota_usb.c): put its first frame up, still.
 * LVGL stops drawing until flash_anim_stop(), and the screen does not go to
 * sleep. False, and nothing changes, without the asset.
 */
bool flash_anim_start(void);

/*
 * After flash_anim_start: loop it from a task of its own until
 * flash_anim_stop(). Started once the slot is erased: the erase stops both
 * cores for most of its ~1.5 s, and the first round of the loop stuttered.
 */
void flash_anim_run(void);

/*
 * End flash_anim_start's loop. for_restart = false: LVGL gets the screen back
 * (the transfer failed). true: the frame on the panel stays, the backlight is
 * latched on through esp_restart(), and the next boot leaves the frame up
 * until its boot clip starts (flash_anim_resuming).
 */
void flash_anim_stop(bool for_restart);


/*
 * At boot, before board_display_init: whether the last run stopped for a
 * restart into a new image. Then the panel still shows that frame and must
 * not be reset. True once per such restart.
 */
bool flash_anim_resuming(void);

/*
 * Every boot, from ui_init once LVGL has the display and under its lock:
 * play the boot clip from a task on core 1 while the UI is built behind it,
 * with nothing of LVGL's drawn until the clip has ended. Without the clip
 * LVGL draws at once, as before.
 */
void flash_anim_boot(lv_display_t *disp);

#ifdef __cplusplus
}
#endif
