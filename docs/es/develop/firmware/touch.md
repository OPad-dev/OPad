---
title: Subsistema táctil del firmware
description: Referencia de implementación del controlador táctil I2C CST816S, tarea touch_retry en Core 1, clasificador de gestos y etiquetas de retroalimentación de deslizamientos.
sidebar:
  order: 4
---

El subsistema táctil controla el controlador táctil capacitivo Hynitron CST816S por I2C para implementar el tercer botón de OPad ([/AGENTS.md](/AGENTS.md) §1.4). Todo el código táctil se ejecuta en **Core 1** con prioridad `2` dentro de `touch_retry_task` ([/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c)).

## Hardware y configuración I2C

- **Controlador**: CST816S en `CST816_I2C_ADDR = 0x15` en `I2C_NUM_0`, `CST816_I2C_FREQ_HZ = 100000` (100 kHz) ([/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c)).
- **Pines** ([/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h](/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h)):
  - `SDA`: **`GPIO 48`** (`BOARD_I2C_SDA_GPIO`)
  - `SCL`: **`GPIO 47`** (`BOARD_I2C_SCL_GPIO`)
  - `INT`: **`GPIO 46`** (`BOARD_TOUCH_INT_GPIO`, flanco descendente activo-bajo)
- **Reposo automático desactivado**: Escribe `0xFF` en `CST816_REG_AUTO_SLEEP` (`0xFE`) para que el controlador nunca añada retardos de despertar de varios milisegundos.

## Bucle reposo por interrupción / sondeo activo

1. **Reposo**: `touch_retry_task` bloquea en un semáforo FreeRTOS señalizado por la ISR de flanco descendente en `GPIO 46`—cero tráfico de CPU o I2C sin tocar la pantalla.
2. **Dedo activo**: Sondea cada `TOUCH_POLL_MS = 10` ms leyendo `CST816_REG_TOUCH_NUM` y coordenadas (`CST816_REG_TOUCH_XH`).
3. **Antirrebote de liberación**: Exige `TOUCH_RELEASE_READS = 2` lecturas vacías consecutivas (~20 ms) antes de soltar para que una lectura I2C ruidosa nunca corte una pulsación sostenida de Quick Retry.
4. **Reserva de calibración**: Mientras un teclado Hall Effect sin calibrar está bloqueado (D7) o calibrándose, `touch_retry.c` reenvía las muestras táctiles exclusivamente a `he_cal_ui_touch_sample` y no envía teclas HID ni deslizamientos.

## Mapeo de coordenadas y clasificación de gestos (`gesture.c`)

El sensor CST816S está montado en vertical (`240x320`) mientras la interfaz corre en horizontal (`320x240`), por lo que las coordenadas se mapean mediante `TOUCH_SWAP_XY = 1` y `TOUCH_MIRROR_Y = 1` ([/firmware/main/input/touch_retry.c](/firmware/main/input/touch_retry.c)).

En [/firmware/main/input/gesture.h](/firmware/main/input/gesture.h) y [/firmware/main/input/gesture.c](/firmware/main/input/gesture.c):
- Umbral `GESTURE_SWIPE_PX = 30` px desde el origen de contacto (`gesture_begin`) antes de clasificar un movimiento como deslizamiento (`gesture_feed`).
- **Dominancia de eje 3:2**: el deslizamiento vertical requiere `2 * |dy| >= 3 * |dx|`; el horizontal requiere `2 * |dx| >= 3 * |dy|`; las diagonales esperan más desplazamiento.
- **Comportamiento en mapa (`PLAYING`)**: Retiene Quick Retry hasta `PLAY_SWIPE_WINDOW_US = 250,000` µs (250 ms) para distinguir un dedo apoyado de un deslizamiento intencionado. Si el dedo se levanta antes de 250 ms sin deslizar, `tap_retry()` envía un toque corto de Quick Retry; si se mantiene pasado 250 ms, Quick Retry permanece pulsado hasta levantar el dedo. Los deslizamientos durante un mapa cancelan Quick Retry y nunca inyectan Alt ni teclas.
- **Deslizamientos de volumen y multimedia**: Enviados directamente por el teclado vía HID (`usb_media.c` para multimedia/volumen de consumo y rueda de ratón; `usb_hid.c` para Alt+rueda del volumen de osu! fuera de mapas, con `release_alt()` esperando hasta `TOUCH_KEY_WAIT_MS = 30` ms para que la pulsación/liberación de F24 despeje la barra de menú Alt de Windows).

## Etiquetas de retroalimentación de deslizamientos (`swipe_labels.c`)

Fuera de partida, [/firmware/main/input/swipe_labels.c](/firmware/main/input/swipe_labels.c) asigna a las acciones de deslizamiento y atajos de osu! (por ejemplo, `F1` Mods, `F2` Random, `F3` Options, volumen, controles multimedia) etiquetas cortas en pantalla (`SWIPE_LABELS`). Durante `PLAYING` no se dibuja ningún distintivo.
