---
title: Subsistema USB del firmware
description: Referencia de implementación de los descriptores compuestos TinyUSB, endpoints HID divididos a 1000 Hz, entramado CDC-ACM y reinicio en modo descarga ROM.
sidebar:
  order: 3
---

El subsistema USB implementa un dispositivo compuesto USB 2.1 Full-Speed (`VID 0x303A`, `PID 0x4001`, `bcdDevice 0x0102`) mediante TinyUSB (`espressif/esp_tinyusb` 2.3.0 / `tinyusb` 0.21.0~2) fijado en Core 0 con prioridad `configMAX_PRIORITIES - 2` ([/firmware/main/app_main.c](/firmware/main/app_main.c), [/firmware/main/usb/usb_descriptors.c](/firmware/main/usb/usb_descriptors.c)).

## Descriptores compuestos y topología de interfaces

Declarados en [/firmware/main/usb/usb_descriptors.c](/firmware/main/usb/usb_descriptors.c):

| Interfaz | Clase / Protocolo | Endpoints | Sondeo | Propósito |
|---|---|---|---|---|
| **Interfaz 0 (`ITF_NUM_HID`)** | Teclado HID (`HID_ITF_PROTOCOL_KEYBOARD`) | `EP 0x81` (IN, 8 B) | `1 ms` (1000 Hz) | Key 1 y Quick Retry táctil (`LINK_KEY1_TOUCH`, report ID `0`) |
| **Interfaces 1 y 2 (`ITF_NUM_CDC`)** | CDC-ACM | `EP 0x82` (Notif IN, 8 B), `EP 0x03` (OUT, 64 B), `EP 0x83` (IN, 64 B) | Bulk / Interrupt | Protocolo serie nanopb entramado (`usb_cdc.c`) |
| **Interfaz 3 (`ITF_NUM_MEDIA`)** | HID Media y Key 2 (`HID_ITF_PROTOCOL_NONE`) | `EP 0x84` (IN, 16 B) | `1 ms` (1000 Hz) | Reporte de teclado de Key 2 (`MEDIA_REPORT_ID_KEYBOARD`), teclas multimedia de consumo (`MEDIA_REPORT_ID_CONSUMER`) y rueda de ratón (`MEDIA_REPORT_ID_MOUSE`) |

También expuestos:
- **Descriptores BOS + MS OS 2.0** (`opad_usb_bos_desc`, `opad_usb_ms_os_20_desc`): Asignan `FriendlyName = "OPad"` a la Interfaz 1 para que el Administrador de dispositivos de Windows muestre `OPad (COMx)` sin archivo INF personalizado.
- **Guardias `_Static_assert` en compilación**: Detienen la compilación si `CFG_TUD_HID < HID_INSTANCE_MEDIA + 1` o `CFG_TUD_CDC < 1` (detectan un `firmware/sdkconfig` desactualizado).

## Envío HID en endpoints divididos (`usb_hid.c`)

Implementado en [/firmware/main/usb/usb_hid.c](/firmware/main/usb/usb_hid.c):

- **Dos enlaces de teclado independientes (`hid_link_t`)**: `LINK_KEY1_TOUCH` (`HID_INSTANCE_KEYBOARD`, `EP 0x81`) y `LINK_KEY2` (`HID_INSTANCE_MEDIA`, `EP 0x84`). Como el ESP32-S3 tiene como máximo 5 endpoints IN incluido `EP0`, Key 2 comparte el endpoint de la Interfaz 3 en lugar de añadir una tercera interfaz HID.
- **Enrutado dinámico de Key 3 (`usb_hid_set_key3`)**: Enruta Key 3 al enlace que no lleve la otra tecla de juego activa para que dos teclas de juego activas nunca compitan por el mismo sondeo USB de 1 ms.
- **Seguimiento de secuencia sin bloqueos**: Los escritores incrementan `change_seq` antes de `try_submit`; un `tud_hid_n_keyboard_report` exitoso hace avanzar `sent_seq`. Si un endpoint está ocupado, `pending_edge_us` guarda la marca temporal del flanco sin entregar más antiguo y `tud_hid_report_complete_cb` reenvía de inmediato y registra la latencia diferida.

## Entramado CDC-ACM y reinicio a descarga ROM (`usb_cdc.c`)

Implementado en [/firmware/main/usb/usb_cdc.c](/firmware/main/usb/usb_cdc.c):

- **Tarea de recepción (`cdc_proto`)**: Corre en Core 1 (`tskIDLE_PRIORITY + 3`, pila de 6144 B) alimentando los bytes recibidos a `protocol_feed_cdc_bytes` y manejando las bajadas de DTR en `tud_cdc_line_state_cb` (que reinician el entramado y cancelan calibraciones HE iniciadas desde la app).
- **Respaldo de toque a 1200 baudios**: Fijar la codificación de línea a 1200 baudios con DTR en bajo arma un reinicio de respaldo hacia el modo de descarga ROM.
- **Reinicio sin botones a descarga ROM (`reboot_to_rom_download`)**:
  1. Llama primero a `tud_disconnect()` (nunca a `tinyusb_driver_uninstall()`, que libera mutex que otras tareas siguen usando).
  2. Fuerza a bajo el pull-up USB `D+` (`SE0`) durante ~300 ms para que el anfitrión registre una desconexión física real (evitando `error -71` en Linux).
  3. Restaura `dp_pullup = 1` *antes* de limpiar `pad_pull_override` para que el chip no desaparezca del bus.
  4. Establece `RTC_CNTL_FORCE_DOWNLOAD_BOOT` en `RTC_CNTL_OPTION1_REG` y llama a `esp_restart()` para entrar en el bootloader ROM del ESP32-S3 (`303a:1001`). (La salida del modo de descarga ROM después la realiza el flasheador del anfitrión mediante un `WRITE_REG` del watchdog RTC.)
