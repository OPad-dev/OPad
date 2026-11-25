---
title: Actualización de firmware
description: Instrucciones paso a paso para flashear y actualizar el firmware de OPad mediante opadctl o la GUI de escritorio con protección de doble ranura OTA y reversión.
sidebar:
  order: 1
---

OPad actualiza su firmware por USB sin requerir nunca los botones físicos BOOT ni RESET. Las actualizaciones escriben en la partición OTA inactiva cuando se conoce la ranura activa, sincronizan los contadores de por vida con el anfitrión antes de flashear (mediante el actualizador del daemon) y revierten automáticamente si la nueva imagen se bloquea o falla la configuración USB. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update, /firmware/main/app_main.c#confirm_image_once_usb_works -->

## Qué necesitas

- Tu OPad conectado directamente a un puerto USB del PC (evita hubs poco fiables durante el flasheo). <!-- src: /desktop/crates/opad-update/src/firmware.rs#consent_text -->
- `opad-daemon` en ejecución (para actualizaciones desde la GUI, `opadctl firmware-update` o detección automática de ranura OTA en `opadctl flash`). `opadctl flash` también funciona sin el daemon en ejecución, aunque sin detección automática de ranura. <!-- src: /desktop/cli/src/main.rs#Commands -->
- `espflash` disponible (incluido con los paquetes de OPad en `<prefix>/lib/opad/bin/espflash` en Linux o junto a `opadctl.exe` en Windows, o instalado en `PATH` / `%USERPROFILE%\.cargo\bin\espflash.exe`). <!-- src: /desktop/crates/opad-device/src/flash.rs#resolve_espflash -->

## Secuencia de seguridad (actualizaciones gestionadas por el daemon)

Tanto el actualizador de la GUI como `opadctl firmware-update` ejecutan la misma tubería del daemon (`firmware_update::install`). **Update now** pregunta siempre. Con **Install firmware updates automatically** activado (por defecto), el daemon también inicia la misma actualización por sí solo cuando el teclado lleva 30 minutos sin usarse: sin pulsaciones y, salvo que la pantalla no se apague nunca, con la pantalla apagada. Nunca durante un mapa, y una versión que falló no se vuelve a intentar hasta un día después: <!-- src: /desktop/daemon/src/firmware_update.rs#AUTO_IDLE, /desktop/daemon/src/firmware_update.rs#auto_wait, /desktop/daemon/src/main.rs, /desktop/crates/opad-update/src/firmware.rs -->

1. **Comprobaciones previas**: El daemon verifica que el teclado esté conectado, ejecutándose desde una ranura OTA (`ota_0` u `ota_1`: la actualización va a la otra), sin un aviso pendiente de toma de propiedad o reemplazo, que el daemon esté en modo `IDLE` (nunca `PLAYING`, `COOLDOWN` ni `SYNC`), que el almacenamiento SQLite esté disponible, que el usuario haya dado su consentimiento (Update now) o que las actualizaciones automáticas estén activadas y que la aplicación de escritorio cumpla cualquier versión `min_app` requerida por el manifiesto. <!-- src: /desktop/daemon/src/firmware_update.rs#install, /desktop/crates/opad-update/src/firmware.rs#blockers, /desktop/crates/opad-update/src/firmware.rs#is_ota_slot, /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
2. **Sincronización previa de contadores**: Antes de descargar o escribir nada, el daemon sincroniza los contadores de por vida del teclado con la base de datos SQLite local (`perform_sync`). <!-- src: /desktop/daemon/src/firmware_update.rs#install -->
3. **Verificación de firma, hash y cabecera**: El manifiesto de lanzamiento se verifica con una firma minisign contra la clave del feed (`trusted_keys`; la clave pública de pruebas solo donde `OPAD_TRUST_TEST_KEY` lo permite), el binario descargado se comprueba contra su hash SHA-256 del manifiesto (`stage_bytes`), y la cabecera de la imagen se verifica buscando el byte mágico ESP `0xE9` y el ID de chip ESP32-S3 `0x0009` (`check_esp32s3_image`). <!-- src: /desktop/crates/opad-update/src/verify.rs#trusted_keys, /desktop/daemon/src/firmware_update.rs#install, /desktop/crates/opad-device/src/flash.rs#check_esp32s3_image -->
4. **Recomprobación final de reposo**: Al terminar la descarga, `still_safe_to_flash` vuelve a comprobar que el daemon sigue en `IDLE`, que el teclado sigue conectado y que no ha empezado ningún mapa durante la descarga. <!-- src: /desktop/daemon/src/firmware_update.rs#still_safe_to_flash -->
5. **Flasheo en ranura opuesta y verificación**: El daemon libera el puerto serie, flashea la imagen de aplicación en la ranura OTA inactiva (`plan_app_update`), escribe el registro `otadata` de prueba, reinicia el teclado (`reset_to_app`), reanuda el descubrimiento del dispositivo y espera hasta 45 segundos (`RECONNECT_TIMEOUT`) a que el teclado vuelva a conectarse informando la ranura objetivo y la nueva versión. <!-- src: /desktop/daemon/src/firmware_update.rs#flash_and_verify, /desktop/crates/opad-device/src/flash.rs#plan_app_update -->

## Actualización desde la GUI de escritorio

Cuando se publica una versión más reciente del firmware en el canal de actualización seleccionado (`stable` o `beta`): <!-- src: /desktop/gui/src/pages.rs#updates, /desktop/gui/src/pages.rs#firmware_updates -->

1. Abre `opad-gui`, selecciona **Settings** en la barra lateral y haz clic en la pestaña **🔄 Updates & Firmware**. (También puedes hacer clic en **Update firmware** en la página **Device** para saltar a **Settings**.) <!-- src: /desktop/gui/src/pages.rs#settings, /desktop/gui/src/pages.rs#device -->
2. En la tarjeta **FIRMWARE UPDATE**, revisa la versión instalada, `Running slot`, la versión disponible, las notas de la versión y cualquier bloqueo `⚠`. <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->
3. Haz clic en **Update firmware** (habilitado solo cuando hay una actualización disponible y no existen bloqueos). <!-- src: /desktop/gui/src/pages.rs#firmware_updates -->
4. Lee el diálogo **Confirm Firmware Update**, que avisa de que el teclado no podrá usarse durante unos 30 segundos (`TYPICAL_OUTAGE_SECONDS = 30`) y advierte que no lo desconectes. Haz clic en **Confirm & Flash Pad**. <!-- src: /desktop/gui/src/main.rs#App, /desktop/crates/opad-update/src/firmware.rs#TYPICAL_OUTAGE_SECONDS -->
5. Espera durante el diálogo modal **Flashing Firmware in Progress** hasta que aparezca **✓ Firmware Update Complete**, y luego haz clic en **Close**. <!-- src: /desktop/gui/src/main.rs#App -->

## Actualización mediante `opadctl` (CLI)

### Actualización de lanzamiento oficial (`opadctl firmware-update`)

Para instalar la versión de firmware anunciada por el manifiesto verificado del daemon usando la misma secuencia de seguridad que la GUI: <!-- src: /desktop/cli/src/main.rs#Commands -->

```sh
opadctl firmware-update
```

Revisa la versión, las notas y el texto de consentimiento, y responde `y` en `Flash the pad now? [y/N]:` (o pasa `--yes` en un script no interactivo). <!-- src: /desktop/cli/src/main.rs#Commands -->

### Flasheo de binario local (`opadctl flash`)

Para instalar un archivo `.bin` compilado o descargado localmente: <!-- src: /desktop/cli/src/main.rs#Commands -->

```sh
opadctl flash firmware/build/opad-firmware.bin
```

Pasa `--port <port>` para anular la autodetección, o `--full <dir>` con un directorio de compilación de ESP-IDF o un directorio de lanzamiento desempaquetado para escribir el bootloader (`0x0`), la tabla de particiones (`0x8000`), `ota_data_initial.bin` (`0xf000`) y la aplicación (`0x20000`). <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/cli/src/main.rs#resolve_flash_set -->

Qué ejecuta `opadctl flash` para una actualización de aplicación:

1. Valida la cabecera de la imagen (`0xE9`, ID de chip ESP32-S3 `0x0009`). <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-device/src/flash.rs#check_esp32s3_image_file -->
2. Consulta a `opad-daemon` la ranura activa (`ota_0` u `ota_1`) y planifica una escritura en la ranura opuesta (`OTA_0` en `0x20000` u `OTA_1` en `0x220000`) más un `otadata` de prueba (`0xf000`). Si la ranura activa se desconoce (sin daemon o firmware antiguo), recurre a escribir `ota_0` en su sitio sin respaldo. <!-- src: /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
3. Envía `IpcRequest::PrepareFlash` para que `opad-daemon` libere el puerto serie CDC. <!-- src: /desktop/cli/src/main.rs#prepare_flash -->
4. Activa el modo de descarga ROM (`enter_bootloader` mediante `BOOTLOADER\n` o toque a 1200 baudios; firmware `reboot_to_rom_download`). <!-- src: /desktop/crates/opad-device/src/flash.rs#enter_bootloader, /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
5. Escribe las imágenes con `espflash write-bin --chip esp32s3 --before no-reset --after no-reset-no-stub --non-interactive` y reinicia el teclado mediante un reinicio del watchdog RTC (`reset_to_app`). <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images, /desktop/crates/opad-device/src/flash.rs#reset_to_app -->
6. Envía `IpcRequest::FinishFlash` para que `opad-daemon` se reconecte, y espera hasta 45 segundos (`ROLLBACK_WAIT`) para confirmar la ranura objetivo o detectar una reversión. <!-- src: /desktop/cli/src/main.rs#finish_flash, /desktop/cli/src/main.rs#report_slot_after_flash -->

## Comportamiento de reversión

Una imagen OTA recién flasheada arranca una vez en estado `ESP_OTA_IMG_PENDING_VERIFY`. Al final de `app_main`, `confirm_image_once_usb_works` espera a que el anfitrión configure el USB (`tud_mounted()`): <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->

- **Confirmada**: En cuanto el anfitrión configura el USB (`tud_mounted()`), el firmware llama a `esp_ota_mark_app_valid_cancel_rollback()` para confirmar permanentemente la nueva ranura. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->
- **Tiempo agotado de configuración USB**: Si un anfitrión USB se conecta (`tud_connected()`) y no configura el USB en 20 segundos (`IMAGE_CONFIRM_TIMEOUT_US`), el firmware llama a `esp_restart()`. El bootloader marca la ranura de prueba como abortada y arranca la ranura válida anterior. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->
- **Bucle de fallos**: Si la nueva imagen falla y se reinicia antes de llegar a la confirmación, el bootloader revierte a la ranura válida anterior en el siguiente arranque. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works, /firmware/sdkconfig.defaults -->
- **Sin anfitrión conectado**: Si se alimenta con un cargador sin anfitrión USB (`!tud_connected()`), la imagen no revierte; queda pendiente hasta que se conecte a un puerto USB de un ordenador. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->
- Los ajustes, la calibración Hall Effect, la propiedad y los contadores de por vida en NVS no se ven afectados por las actualizaciones ni las reversiones OTA. <!-- src: /firmware/partitions.csv -->

## Cómo comprobar que funcionó

Ejecuta `opadctl status` y verifica: <!-- src: /desktop/cli/src/main.rs#Commands -->

- `ESP32 Device: Connected`
- `Firmware Version:` muestra la versión que flasheaste
- `Running Slot:` muestra `ota_0` u `ota_1`

En la GUI, abre **Device** y comprueba **Firmware** y **Running slot**. <!-- src: /desktop/gui/src/pages.rs#device -->

## Qué hacer si no funcionó

- Si el teclado revirtió a la ranura anterior, sigue funcionando como teclado con su firmware previo; consulta [Recuperación §7.6](../recovery.md).
- Si el flasheo falló o el teclado dejó de enumerar como OPad (`303a:8415`), ejecuta `opadctl recover` como se describe en [Recuperación §7.7](../recovery.md).
- Si el puerto serie está ocupado o fallan los permisos, consulta [Solución de problemas](../troubleshooting.md).
