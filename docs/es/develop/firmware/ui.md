---
title: Subsistema de interfaz (UI) del firmware
description: Referencia de implementación del controlador de pantalla ST7789 SPI2, integración LVGL v9.5, asignación de memoria, almacén de diseños y superposición de calibración Hall Effect.
---

El subsistema de UI renderiza diseños personalizables `320x240`, telemetría en vivo, animaciones de resultados y la superposición de calibración Hall Effect en la LCD IPS ST7789 de 2.0 pulgadas de Waveshare. Todo el renderizado se ejecuta en **Core 1** bajo **LVGL v9.5.0** y `esp_lvgl_port` 2.9.0 ([/firmware/dependencies.lock](/firmware/dependencies.lock), [/firmware/main/ui/ui_port.c](/firmware/main/ui/ui_port.c)).

## Hardware de pantalla y controlador SPI2 (`board_display.c`)

Implementado en [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c) y [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h):

- **Controlador**: LCD IPS ST7789 (`320x240`, RGB565, `LCD_RGB_ENDIAN_RGB`, inversión de color activada, `swap_xy = true`, `mirror_x = true` para orientación horizontal).
- **Bus**: `SPI2_HOST` con `SPI_DMA_CH_AUTO` a `BOARD_LCD_PIXEL_CLOCK_HZ = 80 MHz` (~15 ms de transferencia de fotograma completo). Las interrupciones DMA de SPI están fijadas en Core 1 (`ESP_INTR_CPU_AFFINITY_1`) para que las transferencias LCD nunca interrumpan la entrada en Core 0.
- **Pines**: `SCLK = GPIO 39`, `MOSI = GPIO 38`, `DC = GPIO 42`, `CS = GPIO 45`, retroiluminación `BL = GPIO 1` (LEDC PWM, `board_display_set_brightness`).
- **Inyección de fallos**: `CONFIG_OPAD_TEST_FAIL_LCD` fuerza a `board_display_init` a devolver `ESP_FAIL` para verificar el funcionamiento como teclado sin pantalla.

## Memoria de LVGL y bucle de renderizado (`ui_port.c`)

- **Pool de memoria (`CONFIG_LV_MEM_SIZE_KILOBYTES = 160`)**: Pool dedicado de 160 KB para que LVGL nunca compita con el heap del sistema ([/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)). Por defecto (`CONFIG_OPAD_LVGL_PSRAM=n`) este pool vive en RAM interna; activar `CONFIG_OPAD_LVGL_PSRAM=y` mueve el pool de LVGL a los 8 MB de Octal PSRAM mientras **los búferes de dibujo LCD permanecen siempre en RAM interna compatible con DMA** ([/firmware/main/Kconfig.projbuild](/firmware/main/Kconfig.projbuild)).
- **Temporizador de UI de 20 ms (`pad_timer_cb`)**: Corre dentro de la tarea LVGL en Core 1 (`PAD_TIMER_PERIOD_MS = 20`), actualizando fuentes locales del teclado (contadores de por vida/mapa, destello de tecla de 80 ms, KPS rodante de 1 s, reloj/fecha, estado PC/`tosu`) y el reposo/despertar de pantalla (`update_sleep`).

## Motor de diseños y persistencia diferida (`ui_store.c`, `ui_screen.c`)

- **Dos pantallas**: `UI_SCREEN_IDLE` y `UI_SCREEN_PLAYING`, cada una con hasta 32 widgets y fuentes Montserrat de 12/14/16/20/24/32/48 pt ([/firmware/main/ui/core/ui_screen.c](/firmware/main/ui/core/ui_screen.c), [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults)).
- **Almacén NVS ([/firmware/main/ui/ui_store.c](/firmware/main/ui/ui_store.c))**: Guarda diseños en `NVS_NS_UI` (`opad_ui`) bajo las claves `"layout0"` / `"layout1"` envueltos en `layout_blob_t` (`LAYOUT_BLOB_VERSION = 1`).
- **Invariante de flash en partida**: `ui_store_save` y `ui_store_erase` difieren las escrituras en `s_pending_layouts` / `s_dirty_save_mask` / `s_dirty_erase_mask` siempre que `runtime_get_state() != OPAD_STATE_IDLE`, y `ui_store_flush_dirty` las guarda en `IDLE` (reintentando escrituras fallidas tras `FLUSH_RETRY_US = 10 s`).

## Superposición de calibración Hall Effect (`he_cal_ui.c`)

Implementada en [/firmware/main/ui/he_cal_ui.c](/firmware/main/ui/he_cal_ui.c) sobre `lv_layer_top()` (temporizador LVGL de 20 ms, solo inicializada cuando `he_backend_running()`):
- Muestra la pantalla roja de bloqueo **Keys not calibrated** (`HE_LOCK_SCREEN_WAIT_FOR_APP` durante 5 s, luego `HE_LOCK_SCREEN_NO_APP_BUTTON` con **Calibrate here** si no se conecta ninguna app compatible, o `HE_LOCK_SCREEN_USE_APP` cuando está conectada).
- Muestra la captura de posición de reposo, 5 casillas numeradas (`1`–`5`) que pasan a verde a lo largo de 5 pulsaciones completas por tecla (Keys 1–2 en V1, Keys 1–3 en V3), confirmación verde **Done!** de 1 s o motivo rojo **Calibration failed** de 2 s.
