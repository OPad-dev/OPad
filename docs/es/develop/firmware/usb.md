---
title: Subsistema USB del Firmware
description: Referencia de implementación para los descriptores compuestos de TinyUSB, endpoints HID divididos a 1000 Hz, enmarcado serie CDC-ACM y gestión de reinicio para descarga en ROM.
category: reference
source_files:
  - /firmware/main/usb/usb_descriptors.c
  - /firmware/main/usb/usb_hid.c
  - /firmware/main/usb/usb_cdc.c
  - /firmware/main/usb/usb_media.c
---

El subsistema USB implementa un dispositivo compuesto USB 2.0 Full-Speed en el ESP32-S3 utilizando TinyUSB. Expone dos endpoints USB HID independientes y un puerto serie virtual USB CDC-ACM. <!-- src: /firmware/main/usb/usb_descriptors.c -->

## Composite Descriptors & Interface Topology

El descriptor de configuración USB define tres interfaces diferenciadas: <!-- src: /firmware/main/usb/usb_descriptors.c -->

| Índice de Interfaz | Subclase / Protocolo | Endpoint | Dirección | Intervalo de Sondeo | Propósito |
|---|---|---|---|---|---|
| **Interfaz 0 (HID Boot)** | Boot Keyboard | Endpoint 1 | IN | 1 ms (1000 Hz) | Informes de Key 1 y Quick Retry táctil. <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY1_TOUCH --> |
| **Interfaz 1 (HID Media)** | Teclado Personalizado y Consumidor | Endpoint 2 | IN | 1 ms (1000 Hz) | Informes de Key 2 y teclas de gestos multimedia. <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY2 --> |
| **Interfaz 2 y 3 (CDC)** | Control Serie CDC-ACM | Endpoint 3 y 4 | IN / OUT | Bulk | Comunicación bidireccional sobre wire de Protobuf. <!-- src: /firmware/main/usb/usb_cdc.c --> |

Una aserción en tiempo de compilación comprueba que TinyUSB se compile con suficientes instancias HID para que coincida con `usb_descriptors.c`: <!-- src: /firmware/main/usb/usb_descriptors.c -->
```c
_Static_assert(CFG_TUD_HID >= HID_INSTANCE_MEDIA + 1,
               "CONFIG_TINYUSB_HID_COUNT must match the descriptors");
```

## Split Endpoint Architecture (usb_hid.c)

Los teclados USB HID estándar a 1000 Hz comparten un único endpoint. Si se activan teclas alternas en menos de 1 ms, la segunda tecla debe esperar a que el anfitrión complete la transferencia anterior. OPad elimina por completo esta fluctuación (jitter) por serialización: <!-- src: /firmware/main/usb/usb_hid.c -->

- **Endpoint 1 (Interfaz Teclado)**: Transmite informes de teclado de arranque de 8 bytes con Key 1 y Quick Retry táctil (`0x35`).
- **Endpoint 2 (Interfaz Media)**: Transmite informes de teclado independientes con Key 2.
- **Emparejamiento Dinámico de Key 3 (`usb_hid_set_key3_link`)**: En módulos de 3 teclas, Key 3 se enruta dinámicamente al endpoint que no esté ocupando la tecla de juego secundaria. Key 1 y Key 2 nunca compiten por el mismo búfer de endpoint USB. <!-- src: /firmware/main/usb/usb_hid.c -->
- **Estructuras de Enlace Atómicas (`hid_link_t`)**: Core 0 (teclado) y Core 1 (táctil) actualizan contadores atómicos `change_seq` y `sent_seq` sin bloquear mutexes en la ruta de entrada. <!-- src: /firmware/main/usb/usb_hid.c#hid_link_t -->

## CDC-ACM Communication & ROM Reboot (usb_cdc.c)

El puerto serie virtual CDC gestiona actualizaciones de configuración, flasheo de firmware y transmisión de telemetría: <!-- src: /firmware/main/usb/usb_cdc.c -->

- **Alimentación de Tramas**: Los bytes recibidos se introducen directamente en el analizador de tramas (`protocol_feed_cdc_bytes`).
- **Reinicio por Pérdida de DTR**: Si el anfitrión baja DTR (por ejemplo, cerrando `opadctl` o desconectando el daemon), el receptor reinicia su búfer de tramas para evitar la desalineación de bytes. <!-- src: /firmware/main/usb/usb_cdc.c -->
- **Reinicio en Modo de Descarga ROM (`reboot_to_rom_download`)**:
  Al recibir la orden de entrar en modo de flasheo por bootloader:
  1. Tira de USB D+ a nivel bajo (estado de bus `SE0`) durante ~300 ms para que el sistema operativo anfitrión registre una desconexión física real.
  2. Vuelve a establecer `dp_pullup` a 1 antes de desactivar la anulación de pull por GPIO.
  3. Llama a `tud_disconnect()` (nunca a `tinyusb_driver_uninstall()`, que libera mutexes activos).
  4. Dispara un reinicio por hardware mediante watchdog RTC (`WRITE_PERI_REG(RTC_CNTL_RESET_STATE_REG, ...)`).
  5. El ESP32-S3 arranca directamente en su modo de descarga ROM interno (`303a:1001`), listo para `espflash` o `opadctl flash`. <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
