---
title: Subsistema de UI del firmware
description: Referencia de implementación del driver de pantalla ST7789 por SPI2, la integración de LVGL 9.5 en el núcleo 1, la colocación en memoria, el temporizador del pad de 20 ms, el motor de layouts portable ui/core, el almacenamiento diferido de layouts y la superposición de calibración Hall Effect.
sidebar:
  order: 5
---

La UI dibuja los layouts personalizables de 320×240 de Idle y Playing, las animaciones de resultado, la respuesta visual de los deslizamientos y la superposición de calibración Hall Effect en la LCD IPS ST7789 de 2,0" de Waveshare. Se ejecuta entera en el **núcleo 1** con LVGL **9.5.0** y `esp_lvgl_port` **2.9.0** ([/firmware/dependencies.lock](https://github.com/OPad-dev/OPad/blob/main/firmware/dependencies.lock)). La ruta del teclado solo la toca mediante `ui_notify_activity()`, un único store atómico de una marca de tiempo ([/firmware/main/ui/ui.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui.h)).

La pantalla es opcional: si `ui_init()` falla, el pad registra `DIAG_EVENT_LCD_INIT_FAILED` y sigue funcionando como teclado sin pantalla (`CONFIG_OPAD_TEST_FAIL_LCD` lo demuestra).

## Archivos

| Archivo | Función |
|---|---|
| [/firmware/main/ui/ui_port.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui_port.c) | `ui_init()`, configuración del port de LVGL, cambio de pantalla, `pad_timer_cb`, suspensión de la pantalla, la API pública `ui_*` que usa la tarea del protocolo |
| [/firmware/main/ui/ui_store.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui_store.c) | Persistencia de layouts en NVS, diferida fuera de `IDLE` |
| [/firmware/main/ui/he_cal_ui.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/he_cal_ui.c) | Pantalla de bloqueo HE y superposición del flujo de calibración |
| [/firmware/main/ui/easter_egg.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/easter_egg.c) | El GIF del "67" (`FREAKY67`, o un mapa que acaba con un PP terminado en 67) |
| [/firmware/main/ui/core/](https://github.com/OPad-dev/OPad/tree/main/firmware/main/ui/core) | Motor portable: modelo de layout (`ui_core.h`), subjects de datos (`ui_data.c`), constructor de pantallas (`ui_screen.c`), animaciones de resultado (`ui_anim.c`), layouts y paleta por defecto (`ui_defaults.c`), IDs de protocolo (`ui_ids.h`). Solo depende de LVGL, así que `opad-ui-preview` compila el mismo código para el Screen Designer del escritorio ([crates](../desktop/crates.md#7-opad-ui-preview)) |
| [/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c](https://github.com/OPad-dev/OPad/blob/main/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_display.c) | Arranque del panel ST7789, suspensión/despertar, brillo |

Los valores de `ui_ids.h` son formato de protocolo compartido con `protocol/opad.proto` y el escritorio: solo se añaden, nunca se renumeran. Las fuentes de datos que puede enlazar un layout están en [Personalizar el layout de la pantalla](../../guides/customize-screen-layout.md).

## Hardware de la pantalla (`board_display.c`)

- **Panel**: ST7789, 320×240 RGB565, `LCD_RGB_ENDIAN_RGB`, inversión de color activada; horizontal mediante `swap_xy` y `mirror_x` en el panel, reflejado en `.rotation` en la configuración de pantalla de LVGL. `.swap_bytes = true` da el RGB565 big-endian que quiere el panel.
- **Bus**: `SPI2_HOST`, DMA (`buff_dma`), `BOARD_LCD_PIXEL_CLOCK_HZ = 80 MHz`: un fotograma completo en ~15 ms en lugar de ~31 ms. Está por encima del reloj de escritura nominal de 62,5 MHz del ST7789; se comprobó en un pad sin artefactos ([/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h](https://github.com/OPad-dev/OPad/blob/main/firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h)).
- **Afinidad de interrupciones**: la interrupción SPI del IO del panel se reserva en el núcleo 1 (`isr_cpu_id = ESP_INTR_CPU_AFFINITY_1`), así que las transferencias a la LCD nunca interrumpen al núcleo 0.
- **Pines**: SCLK GPIO39, MOSI GPIO38, DC GPIO42, CS GPIO45, sin línea de reset, retroiluminación GPIO1 por PWM de LEDC (`board_backlight_init`, `board_display_set_brightness`).

## Port de LVGL y bucle de renderizado (`ui_port.c`)

- **Tarea del port**: `lvgl_port_init()` con `task_affinity = 1`, `task_priority = 2`, `task_max_sleep_ms = 500`. El temporizador de tick de `esp_lvgl_port` vive en el núcleo 0, así que `ui_init()` lo detiene (`lvgl_port_stop()`) y fija `lv_tick_set_cb(ui_tick_ms)`: LVGL lee `esp_timer` directamente.
- **Búferes de dibujo**: dos (`double_buffer`) de `UI_SCREEN_W × UI_DRAW_BUF_LINES` = 320×40 píxeles, 25,6 KB cada uno, en RAM interna DMA. Con 20 líneas se ahorraban 25,6 KB, pero la animación de resultado se dibujaba un tercio más lenta.
- **Refresco**: `CONFIG_LV_DEF_REFR_PERIOD = 33` limita los redibujados a 30 fps; los valores cambian como mucho 10 veces por segundo y a 60 fps no se veía ninguna diferencia ([/firmware/sdkconfig.defaults](https://github.com/OPad-dev/OPad/blob/main/firmware/sdkconfig.defaults)).
- **`pad_timer_cb`** (`PAD_TIMER_PERIOD_MS = 20`, en la tarea de LVGL):
  - carga `UI_SCREEN_PLAYING` mientras `runtime_get_state() == OPAD_STATE_PLAYING` y, si no, `UI_SCREEN_IDLE`; fuerza Idle mientras el pad HE está bloqueado o calibrando; detiene una animación de resultado cuando empieza un mapa;
  - lanza el easter egg cuando un mapa acaba con un PP terminado en 67;
  - `update_sleep()`: tras `sleep_s` sin actividad (por defecto 600 s), desactiva la invalidación y suspende el panel; la actividad lo despierta y restaura el brillo;
  - mientras está despierta, `update_pad_sources()`: cuentas de por vida y por mapa de las dos tarjetas de teclas de juego (`game_keys_slots`), un destello de tecla mantenido 80 ms para que se vea un toque, totales, KPS móviles de 1 s, estado de PC/tosu (borra los valores que vienen del daemon cuando se cierra el puerto), tiempo encendido, y hora/fecha una vez por minuto cuando el host ya ha fijado la hora.
- **API del lado del protocolo**: `ui_set_number()`, `ui_set_string()`, `ui_set_layout()`, `ui_play_animation()` (ignorada durante un mapa) toman el bloqueo de LVGL desde la tarea del protocolo; `ui_lock()` / `ui_unlock()` agrupan varias actualizaciones y devuelven false sin pantalla.

## Colocación en memoria

- **Pool de LVGL**: `CONFIG_LV_MEM_SIZE_KILOBYTES = 160`, reservado en PSRAM al arrancar cuando `CONFIG_OPAD_LVGL_PSRAM = y` (por defecto) mediante `LV_MEM_POOL_ALLOC` ([/firmware/main/ui/lvgl_psram_pool.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/lvgl_psram_pool.h)). Con el pool en RAM interna, a un pad HE V3 le quedaba muy poca RAM DMA para los búferes de dibujo y arrancaba con la pantalla en blanco. `easter_egg_init()` añade otro pool de 512 KB en PSRAM para el GIF.
- **Búferes estáticos en PSRAM** (`EXT_RAM_BSS_ATTR`, `CONFIG_SPIRAM_ALLOW_BSS_SEG_EXTERNAL_MEMORY`): `s_layouts` (`ui_port.c`), `s_pending_layouts` (`ui_store.c`), `s_subjects` / `s_str_buf` / `s_str_prev` (`ui_data.c`, tras `#ifdef ESP_PLATFORM` para que la vista previa de escritorio compile sin `esp_attr.h`).
- **Cachés**: 32 KB de instrucciones y 64 KB de datos con líneas de 64 B reducen a la mitad el tiempo de fotograma del código y las fuentes en flash y del pool en PSRAM (pantalla de partida de 29 → 14,5 ms por fotograma, según el comentario de `sdkconfig.defaults`).

## Layouts y persistencia diferida (`ui_store.c`)

- Dos pantallas, `UI_SCREEN_IDLE` y `UI_SCREEN_PLAYING`, cada una con hasta `UI_MAX_WIDGETS = 32` widgets ([/firmware/main/ui/core/ui_core.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/core/ui_core.h)). Al arrancar, `ui_store_load()` lee cada una y recurre a `ui_default_layout()` si falta.
- Se guardan en `NVS_NS_UI` (`opad_ui`) bajo `layout0` / `layout1`, envueltas en `layout_blob_t` (`LAYOUT_BLOB_VERSION = 1`, con el tamaño comprobado frente a `ui_layout_t`).
- **Nada de escrituras en flash fuera de `IDLE`**: `ui_store_save()` y `ui_store_erase()` solo fijan `s_pending_layouts`, `s_dirty_save_mask` y `s_dirty_erase_mask` mientras `runtime_get_state() != OPAD_STATE_IDLE`; `ui_store_flush_dirty()` (que llama la tarea de ejecución) las confirma en `IDLE` y reintenta una escritura fallida tras `FLUSH_RETRY_US = 10 s`.

## Superposición de calibración Hall Effect (`he_cal_ui.c`)

Se construye sobre `lv_layer_top()` con su propio temporizador de LVGL de 20 ms, y solo cuando `he_backend_running()` (los pads MX no tienen temporizador de calibración). Implementa la decisión D7 de [/docs/specs/v2-hall-effect-decisions.md](../../../specs/v2-hall-effect-decisions.md); el flujo de usuario está en [Calibrar Hall Effect](../../guides/calibrate-hall-effect.md).

| Pantalla | Se muestra cuando | Texto |
|---|---|---|
| `HE_LOCK_SCREEN_WAIT_FOR_APP` | Bloqueado, primeros 5 s tras arrancar (`BOOT_APP_WAIT_US`) | **Keys not calibrated** / "Looking for the OPad app..." |
| `HE_LOCK_SCREEN_USE_APP` | Bloqueado, hay conectada una app capaz de calibrar | "Calibrate in the OPad app" / "on your PC"; sin botón |
| `HE_LOCK_SCREEN_NO_APP_BUTTON` | Bloqueado, sin una app capaz | "For the best setup, get the app:" `https://opad.dev/download` (`OPAD_DOWNLOAD_URL`) y un botón **Calibrate here** |
| Reposo | Calibración iniciada | **Calibrating** / "Take your fingers off both keys" (todas las teclas en V3) |
| Tecla *n* | Cada tecla por turno | **Key *n*** / "Press key *n* all the way, 5 times", cinco casillas de progreso |
| Éxito | Todas las teclas capturadas y guardadas | **Done!** / "Your keys are ready", verde, 1 s |
| Fallo | Error de captura, error al guardar o la app se ha desconectado | **Calibration failed** con el motivo, rojo, 2 s; un botón **Try again** solo en un pad bloqueado sin app |

El botón usa `he_cal_button.c` (bajada y subida dentro de él, desplazamiento de menos de 30 px), así que un deslizamiento que lo cruce no hace nada. El bloqueo se levanta en cuanto se guarda la nueva calibración; sin reiniciar.
