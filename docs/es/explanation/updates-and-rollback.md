---
title: Actualizaciones de firmware y reversión automática
description: Disposición de particiones OTA duales, verificación de arranque de prueba del bootloader, reversión automática basada en USB y canales de actualización de software.
sidebar:
  order: 6
---

OPad actualiza su firmware por USB sin tocar los botones físicos BOOT ni RESET (AGENTS.md §1.5). Dos mecanismos impiden que una actualización defectuosa deje inutilizado un teclado: una tabla de particiones OTA de doble ranura con reversión automática condicionada por USB, y una ruta de recuperación por ROM de máscara (`opadctl recover`) cuando una imagen ni siquiera enumera. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /desktop/crates/opad-device/src/flash.rs#recover -->

## Disposición de particiones OTA duales

La tabla de particiones del ESP32-S3 en [`firmware/partitions.csv`](/firmware/partitions.csv) define dos ranuras de aplicación de 2 MB junto a NVS y metadatos de selección OTA (`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y` en [`firmware/sdkconfig.defaults`](/firmware/sdkconfig.defaults)):

| Nombre de partición | Tipo | Subtipo | Desplazamiento Flash | Tamaño | Propósito |
|---|---|---|---|---|---|
| **`nvs`** | Data | NVS | `0x9000` | 24 KB (`0x6000`) | Ajustes del dispositivo, propiedad (`owner_id`), calibración, diseños, contadores de por vida. |
| **`otadata`** | Data | OTA | `0xf000` | 8 KB (`0x2000`) | Dos sectores de 4 KB que registran la ranura OTA activa y el estado de imagen ([`OTADATA_OFFSET`](/desktop/crates/opad-device/src/flash.rs)). |
| **`phy_init`** | Data | PHY | `0x11000` | 4 KB (`0x1000`) | Datos de inicialización RF/PHY. |
| **`ota_0`** | App | OTA_0 | `0x20000` | 2 MB (`0x200000`) | Ranura 0 de aplicación de firmware ([`APP_PARTITION_OFFSET`](/desktop/crates/opad-device/src/flash.rs)). |
| **`ota_1`** | App | OTA_1 | `0x220000` | 2 MB (`0x200000`) | Ranura 1 de aplicación de firmware ([`OTA_1_OFFSET`](/desktop/crates/opad-device/src/flash.rs)). |

Al flashear mediante `opadctl flash` (conociendo la ranura activa) o el actualizador de firmware del daemon ([`plan_app_update()`](/desktop/crates/opad-device/src/flash.rs)):

1. El teclado informa su partición activa en `HelloAck.running_partition` ([`protocol_send_hello_ack()`](/firmware/main/protocol/protocol.c)), que el daemon guarda en `DeviceInfo.running_partition`.
2. [`plan_app_update()`](/desktop/crates/opad-device/src/flash.rs) apunta a la **ranura opuesta** (`ota_1` si corre `ota_0`, u `ota_0` si corre `ota_1`), dejando intacta la ranura funcional. (Nota: el actualizador gestionado por el daemon actualmente comprueba [`is_ota_0()`](/desktop/crates/opad-update/src/firmware.rs) antes de empezar, por lo que solo ofrece actualizaciones cuando el teclado está en `ota_0`.)
3. [`otadata_image()`](/desktop/crates/opad-device/src/flash.rs) construye una imagen `otadata` de 8 KB con dos registros `esp_ota_select_entry_t` y sumas CRC-32 zlib (`otadata_crc`):
   - El sector 0 mantiene la ranura previa marcada como `OTA_IMG_VALID` (`0x2`).
   - El sector 1 selecciona la ranura recién escrita con un `ota_seq` mayor marcado como `OTA_IMG_NEW` (`0x0`).
4. Primero se escribe el binario de aplicación y en segundo lugar `otadata`, de modo que una escritura interrumpida deja seleccionada la ranura antigua. Por último, [`reset_to_app()`](/desktop/crates/opad-device/src/flash.rs) limpia `RTC_CNTL_FORCE_DOWNLOAD_BOOT` y dispara un reinicio por watchdog RTC hacia la nueva imagen.

## Arranque de prueba y reversión controlada por USB

Cuando el bootloader de segunda etapa del ESP32-S3 arranca una ranura `OTA_IMG_NEW`, cambia su estado a `ESP_OTA_IMG_PENDING_VERIFY`. El firmware en ejecución debe confirmarse a sí mismo o el bootloader revertirá en el siguiente reinicio ([`confirm_image_once_usb_works()`](/firmware/main/app_main.c)).

```
[Bootloader Starts]
         |
         v
[Boot New Image (PENDING_VERIFY)]
         |
         +------------------------------------------------------------------+
         |                                                                  |
         v                                                                  v
[Host configures USB (tud_mounted())]             [Crash loop or host does not configure USB within 20 s]
         |                                                                  |
         v                                                                  v
esp_ota_mark_app_valid_cancel_rollback()                             esp_restart() / reset
         |                                                                  |
         v                                                                  v
[Image Confirmed Permanently]                     [Bootloader marks slot aborted and boots previous slot]
```

### Criterios de verificación (`confirm_image_once_usb_works`)

[`confirm_image_once_usb_works()`](/firmware/main/app_main.c) se ejecuta al final de `app_main`, después de inicializar entrada, TinyUSB, CDC, contadores, runtime, pantalla y táctil:

1. **Comprobación de pendiente**: Lee `esp_ota_get_state_partition()` de la partición en ejecución. Si no es `ESP_OTA_IMG_PENDING_VERIFY`, la función retorna de inmediato.
2. **Requisito de configuración USB**: Espera en bucle mientras `!tud_mounted()`, sondeando cada 100 ms, porque un firmware que arranque internamente sin convertirse en un teclado USB HID utilizable no debe confirmarse.
3. **Ventana límite de 20 segundos**: Cuando se conecta un anfitrión USB (`tud_connected()`), arranca un temporizador. Si `tud_mounted()` no pasa a verdadero en 20 segundos (`IMAGE_CONFIRM_TIMEOUT_US = 20 * 1000 * 1000`), el firmware registra un error y llama a `esp_restart()`.
4. **Reversión del bootloader**: Al reiniciar (ya sea por `esp_restart()` o por un bucle de fallos antes de confirmarse), el bootloader del ESP32-S3 ve que la ranura sigue sin confirmar, la marca como abortada y arranca la ranura `OTA_IMG_VALID` anterior.
5. **Sin anfitrión conectado (excepción de cargador)**: Si se alimenta desde un cargador de pared o batería externa, `tud_connected()` permanece falso y `host_since_us` se reinicia a `0`. La imagen no revierte; queda pendiente hasta que el teclado vuelva a conectarse a un puerto USB de un ordenador.
6. **Confirmación permanente**: En cuanto `tud_mounted()` devuelve verdadero, el firmware llama a `esp_ota_mark_app_valid_cancel_rollback()`.

## Liberación del puerto serie y apretón de manos de modo descarga

El flasheo por USB coordina al daemon, al firmware en ejecución y a `espflash` ([`reboot_to_rom_download()`](/firmware/main/usb/usb_cdc.c), [`enter_bootloader()`](/desktop/crates/opad-device/src/flash.rs)):

1. `opadctl flash` envía `IpcRequest::PrepareFlash` (o el actualizador del daemon llama a `device.pause_and_release()`), cerrando el descriptor CDC para liberar el puerto (obligatorio en Windows, donde los puertos serie son exclusivos).
2. [`enter_bootloader()`](/desktop/crates/opad-device/src/flash.rs) intenta hasta dos disparadores (`BOOT_TRIGGERS`): enviar `BOOTLOADER\n` por CDC y después abrir y cerrar el puerto a 1200 baudios (bajando DTR).
3. Dentro de [`reboot_to_rom_download()`](/firmware/main/usb/usb_cdc.c), el firmware vacía CDC TX, espera 50 ms, llama a `tud_disconnect()` (nunca `tinyusb_driver_uninstall()`), redirige el PHY USB interno de vuelta a USB-Serial-JTAG, mantiene SE0 (ambas líneas a bajo, sin pull-up) durante 300 ms para que el anfitrión vea una desconexión real, restaura `dp_pu = true` *antes* de desactivar la anulación de pull, espera hasta 1.5 s `USB_SERIAL_JTAG_INTR_BUS_RESET`, escribe `RTC_CNTL_FORCE_DOWNLOAD_BOOT` en `RTC_CNTL_OPTION1_REG` y llama a `esp_restart()`.
4. [`write_images()`](/desktop/crates/opad-device/src/flash.rs) invoca `espflash write-bin --chip esp32s3 --before no-reset --after no-reset-no-stub --non-interactive` para cada imagen.
5. [`reset_to_app()`](/desktop/crates/opad-device/src/flash.rs) envía comandos SLIP `WRITE_REG` directamente al bootloader ROM para limpiar `RTC_CNTL_OPTION1_REG` y disparar un reinicio por watchdog RTC hacia la aplicación.
6. El daemon reanuda el sondeo (`IpcRequest::FinishFlash` o `device.resume()`) y espera a que el teclado vuelva a conectarse.

## Canales de actualización del software de escritorio

Los componentes de escritorio (la app OPad y el `tosu` incluido) y las ofertas de firmware se coordinan mediante [`opad-update`](/desktop/crates/opad-update/src/lib.rs) y el trabajador de actualizaciones del daemon ([`desktop/daemon/src/updater.rs`](/desktop/daemon/src/updater.rs)):

- **Canales de lanzamiento y programación**: Los usuarios eligen `stable` (por defecto) o `beta` en **Settings** -> **🔄 Updates & Firmware** ([`pages.rs`](/desktop/gui/src/pages.rs)). El trabajador despierta cada 15 minutos (`TICK`), realiza una comprobación diaria con caché ETag únicamente mientras el daemon está en `IDLE` (nunca durante `PLAYING` o `COOLDOWN`), y aplica despliegue gradual mediante el ID de instalación (`rollout::for_install`).
- **Verificación**: Los bytes del manifiesto se verifican con una firma **minisign** separada contra `TRUSTED_KEYS` antes de analizarlos, y cada artefacto descargado se comprueba contra su hash **SHA-256** dentro de ese manifiesto firmado ([`verify.rs`](/desktop/crates/opad-update/src/verify.rs)).
- **Aplicación según origen**: La forma de aplicar una actualización de la app depende del marcador `install-origin` ([`origin.rs`](/desktop/crates/opad-update/src/origin.rs)):
  - `windows`: ejecuta el instalador firmado de Inno Setup con `/SILENT /NORESTART /RELAUNCH=1` ([`apply_command()`](/desktop/crates/opad-update/src/app.rs)) (que detiene el daemon; la recuperación al inicio [`resolve_interrupted()`](/desktop/daemon/src/updater.rs) registra el resultado tras reiniciar).
  - `deb` / `rpm`: actualiza mediante `apt` / `dnf` (usando `pkcon` cuando el repositorio de OPad está configurado, o `pkexec` sobre el paquete descargado).
  - `appimage` / `user`: reemplaza el archivo `$APPIMAGE` de forma atómica (`replace_file`) o extrae el archivo sobre `install_prefix()` con `tar -xzf`.
  - `aur` / `source` / `unknown`: `NotifyOnly` (informa de la versión disponible sin modificar archivos).
- **Autonomía y reinicio**: El `tosu` incluido se autoactualiza cuando está activado en orígenes donde OPad es propietario del binario (`windows`, `user`, `appimage`). La app de escritorio nunca se autoinstala; espera a que el usuario haga clic en **Install now**, y luego activa `restart_required` para que la GUI muestre `"OPad was updated. Restart the app and the daemon to finish — the pad keeps working as a keyboard meanwhile."` ([`main.rs`](/desktop/gui/src/main.rs)). Las actualizaciones de firmware nunca son automáticas y siempre requieren consentimiento explícito.
