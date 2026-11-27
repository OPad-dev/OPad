# Asset demo hang (2026-11-27/28)

A record of what was tried, not current guidance: the code and `git log` win.

## What happened

A test build (`CONFIG_OPAD_ASSET_DEMO`, since removed) played every GIF and
MJPEG of the new asset partitions in a loop over the UI, to compare formats on
the owner's Hall Effect pad. MJPEG frames were decoded with LVGL's TJpgDec
into an RGB565 buffer that an `lv_image` showed; GIFs went through `lv_gif`.

On the panel:

| Clip | Seen |
|---|---|
| `opad_firmware_320x240_24fps.mjpeg` (4:4:4) | black: TJpgDec refuses every frame (`JDR_FMT3`), reproduced on the PC |
| `opad_firmware_12fps.gif`, `..._calidad_esp32.gif` | many black dots |
| `opad_updating_12fps_alta.mjpeg` (4:2:0) | perfect |
| `opad_updating_12fps_media.mjpeg` (4:2:0) | never shown: the pad restarted first |

The restarts were `INT_WDT` resets, every 4–55 s, with nothing in the
coredump partition (the panic handler never got to write it). A breadcrumb in
RTC memory showed them at no fixed point: mid-clip, end of clip, frame 6 or 8.

## Tried

| Build | Hangs? |
|---|---|
| Decode every frame, never draw it (80 s) | no |
| Draw the same frame again at 12 fps, no decoding | yes |
| A 160x120 image in internal RAM instead of PSRAM | yes |
| LVGL pool at 160 KB (the product's) instead of 1 MB | yes (later, 30 s) |
| No asset partition mapped at all | yes |
| Daemon stopped (no NVS writes) | yes, 9 restarts in 80 s |
| Product firmware of the same branch, `opadctl animation burst SS` back to back for 90 s | no |

So an `lv_image` invalidated over and over hung the pad, while LVGL redrawing
the grade animation as often did not. Not explained: whether it is the image
draw path, the 5 ms `lv_timer` that drove the demo, or something else only the
demo build had.

## What was done instead

The flash animation (`firmware/main/ui/flash_anim.c`) bypasses LVGL: it
decodes each frame straight into a 10 KB DMA band and sends it to the panel
itself, for at most 1.2 s before the restart into the ROM bootloader. Anything
that plays an animation through LVGL for longer has to find this cause first.
