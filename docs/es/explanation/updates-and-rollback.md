---
title: Actualizaciones de Firmware y Reversión Automática (Rollback)
description: Esquema de particiones OTA dual, verificación de arranque de prueba en el bootloader, reversión automática guiada por USB y canales de actualización de software.
category: explanation
source_files:
  - /firmware/main/app_main.c
  - /firmware/main/runtime/runtime.c
  - /firmware/main/protocol/protocol.c
  - /firmware/main/usb/usb_cdc.c
  - /desktop/crates/opad-device/src/flash.rs
  - /desktop/crates/opad-update/src/lib.rs
  - /desktop/daemon/src/firmware_update.rs
---

OPad garantiza que una actualización de firmware nunca pueda dejar un teclado inservible o bloqueado (Regla de Producto 5). La actualización del firmware está totalmente automatizada, no requiere interactuar con los botones físicos de hardware BOOT o RESET y cuenta con un mecanismo de reversión automática (rollback): si una imagen recién flasheada falla, se bloquea o no logra enumerar en USB, la ROM del ESP32-S3 y los bootloaders de segunda etapa revierten automáticamente a la ranura de firmware que funcionaba previamente. <!-- src: /firmware/main/app_main.c -->

## Dual OTA Partition Layout

La tabla de particiones flash del ESP32-S3 define dos ranuras de aplicación alternas junto con metadatos de selección de arranque: <!-- src: /desktop/crates/opad-device/src/flash.rs -->

| Nombre de Partición | Tipo de Partición | Subtipo | Desplazamiento Flash | Tamaño | Propósito |
|---|---|---|---|---|---|
| **`nvs`** | Data | NVS | `0x9000` | 24 KB | Ajustes del dispositivo, calibración, contadores de por vida. |
| **`otadata`** | Data | OTA | `0xf000` | 8 KB (`0x2000`) | Dos sectores de 4 KB que rastrean la ranura OTA activa y el estado de la imagen. <!-- src: /desktop/crates/opad-device/src/flash.rs#OTADATA_OFFSET --> |
| **`ota_0`** | App | OTA_0 | `0x20000` | 2 MB | Ranura de aplicación de firmware 0. |
| **`ota_1`** | App | OTA_1 | `0x220000` | 2 MB | Ranura de aplicación de firmware 1. <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET --> |

Al flashear a través de `opadctl flash` o del actualizador de escritorio: <!-- src: /desktop/crates/opad-device/src/flash.rs -->
1. El anfitrión consulta `DeviceInfo.running_partition` por serie CDC (por ejemplo, `ota_0`). <!-- src: /desktop/crates/opad-device/src/flash.rs -->
2. El flasheador apunta a la **ranura opuesta** (`ota_1`), manteniendo el firmware funcional actual sin modificaciones en la flash.
3. El flasheador genera una nueva imagen `otadata` (`otadata_image`) con dos entradas de arranque:
   - Sector 0: Mantiene la ranura antigua marcada como `OTA_IMG_VALID` (`0x2`). <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_IMG_VALID -->
   - Sector 1: Apunta a la nueva ranura marcada como `OTA_IMG_NEW` (`0x0`, arranque de prueba). <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_IMG_NEW -->
4. Se ordena al dispositivo reiniciarse en la nueva imagen.

## Trial Boot & USB-Driven Rollback

Cuando arranca el bootloader del ESP32-S3, inicia la ranura recién flasheada en estado `ESP_OTA_IMG_PENDING_VERIFY`. El nuevo firmware debe validarse explícitamente a sí mismo o se enfrentará a la reversión. <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->

```
[Inicia Bootloader]
         |
         v
[Arranca Nueva Imagen (PENDING_VERIFY)]
         |
         +--------------------------------------------------------+
         |                                                        |
         v                                                        v
[USB Configurado en <= 20s]                           [Bucle de Bloqueos o Cuelgue USB > 20s]
         |                                                        |
         v                                                        v
esp_ota_mark_app_valid_cancel_rollback()                      esp_restart()
         |                                                        |
         v                                                        v
[Imagen Confirmada Permanentemente]                   [Bootloader Revierte a Ranura Previa]
```

### Verification Criteria (confirm_image_once_usb_works)
La lógica de confirmación de imagen del firmware se ejecuta al final de `app_main.c`: <!-- src: /firmware/main/app_main.c -->
1. **Comprobación de Pendiente**: La imagen comprueba si su partición en ejecución está en `ESP_OTA_IMG_PENDING_VERIFY`. Si ya fue validada, continúa inmediatamente. <!-- src: /firmware/main/app_main.c -->
2. **Requisito de Enumeración USB**: Una imagen que arranca y ejecuta tareas internas pero no logra enumerar como un teclado USB HID es inútil para el jugador. El pad espera a que el anfitrión USB monte el dispositivo (`tud_mounted()`). <!-- src: /firmware/main/app_main.c -->
3. **Ventana de Tiempo Límite de 20 Segundos**: Si se detecta una conexión de anfitrión USB (`tud_connected()`) pero el anfitrión no logra configurar la pila TinyUSB HID en 20 segundos (`IMAGE_CONFIRM_TIMEOUT_US = 20,000,000`), el firmware registra un error y dispara `esp_restart()`. <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->
4. **Reversión del Bootloader**: Al reiniciarse, el bootloader del ESP32 detecta que la imagen de prueba falló la verificación, la marca como no válida y arranca de vuelta en la partición válida anterior.
5. **Sin Anfitrión Conectado (Excepción de Cargador)**: Si el pad se alimenta desde un cargador de pared en lugar de un anfitrión USB de ordenador, `tud_connected()` permanece en false. El pad no revierte; la verificación se mantiene pendiente hasta que el pad se conecte a un anfitrión de ordenador. <!-- src: /firmware/main/app_main.c -->
6. **Validación**: Tan pronto como `tud_mounted()` tiene éxito, el pad llama a `esp_ota_mark_app_valid_cancel_rollback()`, confirmando permanentemente la ranura. <!-- src: /firmware/main/app_main.c -->

## Serial Port Release & Download Mode Handshake

Al flashear sobre USB CDC serie (`reboot_to_rom_download` en `firmware/main/usb/usb_cdc.c` y `desktop/crates/opad-device/src/flash.rs`): <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
- `opadctl flash` envía `IpcRequest::PrepareFlash` a `opad-daemon`.
- El daemon cierra su descriptor CDC, liberando el acceso a `/dev/ttyACM0` o al puerto COM.
- El flasheador ordena al firmware entrar en el modo de descarga de la ROM. El firmware aplica la desconexión del pull-up de USB D+ (estado de línea `SE0` durante ~300 ms), restablece `dp_pullup` a 1, llama a `tud_disconnect()` y dispara un reinicio por watchdog RTC de hardware. <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
- El flasheo continúa mediante `espflash` directamente en la partición OTA inactiva.
- Tras concluir el flasheo, `opad-daemon` reanuda el sondeo (`IpcRequest::FinishFlash`), descubriendo el nuevo firmware arrancado.

## Desktop Software Update Channels

Las aplicaciones de escritorio (`opad-daemon`, `opad-gui`, `opadctl` y `tosu`) están gestionadas por `opad-update`: <!-- src: /desktop/crates/opad-update/src/lib.rs -->
- **Canales de Publicación**: Los usuarios pueden elegir entre los canales `stable` y `beta` en los ajustes.
- **Firmas de Manifiestos**: Los manifiestos de actualización se comprueban con firmas criptográficas. Los archivos binarios se descargan en directorios temporales de preparación y se descomprimen atómicamente sobre el prefijo de instalación.
- **Supervisión Independiente**: El motor de actualización sustituye los binarios de la aplicación en disco mientras los procesos siguen ejecutándose; la interfaz gráfica muestra una notificación discreta de "Reiniciar para aplicar actualización".
