---
title: Subsistema de Interfaz (UI) del Firmware
description: Referencia de implementación para el controlador de pantalla ST7789, integración con LVGL, búferes de pantalla en PSRAM y arquitectura del almacén de diseños.
---

El subsistema de interfaz de usuario renderiza widgets de telemetría HUD dinámicos, metadatos de canciones, animaciones de calificación de celebración y asistentes guiados de calibración en la pantalla LCD Waveshare ST7789 de 2.0 pulgadas (resolución de 320x240). Todo el renderizado de la pantalla se ejecuta en Core 1 bajo LVGL v8, utilizando grupos de memoria en PSRAM para proteger la SRAM interna y la latencia de las teclas. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c -->

## Display Hardware & SPI Driver (board_display.c)

- **Controlador**: Controlador LCD IPS Sitronix ST7789. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c -->
- **Interfaz**: Host SPI2 (`BOARD_LCD_SPI_HOST`) funcionando a alta frecuencia de reloj con transferencias DMA por hardware.
- **Asignaciones de Pines**:
  - `SCLK`: GPIO 39
  - `MOSI`: GPIO 38
  - `DC`: GPIO 42
  - `CS`: GPIO 45
  - `Retroiluminación (BL)`: GPIO 1 (controlada por temporizador PWM LEDC para un ajuste granular de brillo del 0 al 100%). <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c#board_display_set_brightness -->

## LVGL Integration & PSRAM Memory Pool

- **Asignación en PSRAM (`CONFIG_OPAD_LVGL_PSRAM`)**: El grupo de memoria interno de LVGL (`lv_mem_init`) y los búferes de dibujo RGB565 de doble búfer se asignan desde la PSRAM externa de 2 MB mediante `opad_lvgl_pool_alloc` (`ui/lvgl_psram_pool.h`). Esto mantiene los 512 KB de SRAM interna completamente libres para las tareas de FreeRTOS y las pilas ISR de alta prioridad. <!-- src: /firmware/CMakeLists.txt -->
- **Rotación de Pantalla**: El panel de hardware ST7789 está montado verticalmente (240x320); `ui_port.c` rota el contexto de visualización mediante `swap_xy` y `mirror_x` para presentar una interfaz apaisada de 320x240 sin penalización de rotación por software.

## Layout Engine & UI Store (ui_store.c & ui_screen.c)

Los diseños de pantalla están estructurados como colecciones de hasta 32 widgets dinámicos (`ui_widget_t`): <!-- src: /firmware/main/ui/ui_store.c -->

- **Perfiles Admitidos**:
  - `UI_SCREEN_IDLE`: Renderizado durante la navegación en menús o el uso en escritorio.
  - `UI_SCREEN_PLAYING`: Renderizado durante las partidas activas de osu!.
- **Propiedades de los Widgets (`ui_screen.c`)**:
  - Fuentes de datos vinculadas (combo, precisión, UR, PPM, artista/título de la canción).
  - Tipografía configurable (tabla `s_fonts`: Montserrat 12, 14, 16, 20, 24, 32, 48 pt). <!-- src: /firmware/main/ui/core/ui_screen.c#s_fonts -->
  - Alineaciones de texto (`LV_TEXT_ALIGN_LEFT`, `CENTER`, `RIGHT`). <!-- src: /firmware/main/ui/core/ui_screen.c#s_text_align -->
  - Banderas visuales (`FLAG_BORDER`, `FLAG_BG_FILL`, `FLAG_HIDE_WHEN_EMPTY`).
- **Persistencia Diferida en NVS**: Cuando el anfitrión envía `SetLayout` durante un mapa, el diseño se almacena temporalmente en RAM (`s_pending_layout`). Los diseños solo se escriben en NVS cuando el dispositivo regresa a `IDLE` (`ui_store_flush_dirty`), asegurando que los ciclos de borrado en flash nunca coincidan con el juego activo. <!-- src: /firmware/main/ui/ui_store.c -->

## Guided Calibration UI (he_cal_ui.c)

Cuando la calibración de Hall Effect está activa: <!-- src: /firmware/main/ui/he_cal_ui.c -->
- Las pantallas normales de HUD se suprimen y se crea la pantalla del asistente de calibración (`he_cal_ui`).
- Renderiza instrucciones paso a paso (asentamiento de posición de reposo, captura de fondo para Key 1 y Key 2).
- Muestra barras de progreso en vivo que reflejan lecturas ADC en bruto y errores (`HE_CAL_ERR_REST_NOT_STILL`, `HE_CAL_ERR_SPAN_TOO_SMALL`).
- La entrada táctil en esta pantalla está restringida estrictamente a los botones de calibración, suprimiendo por completo el Quick Retry táctil y los gestos de deslizamiento habituales. <!-- src: /firmware/main/ui/he_cal_ui.c -->
