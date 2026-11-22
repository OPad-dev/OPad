---
title: Actualización del firmware
description: Instrucciones paso a paso para flashear y actualizar el firmware de OPad mediante opadctl o la GUI de escritorio con protección de doble ranura OTA y reversión automática.
category: how-to
source_files:
  - /desktop/cli/src/main.rs
  - /desktop/crates/opad-device/src/flash.rs
  - /desktop/daemon/src/firmware_update.rs
  - /desktop/daemon/src/updater.rs
  - /desktop/crates/opad-update/src/firmware.rs
  - /firmware/main/usb/usb_cdc.c
  - /firmware/main/app_main.c
  - /firmware/main/protocol/protocol.c
  - /firmware/main/runtime/runtime.c
---

Actualizar el firmware de OPad es un proceso completamente seguro y gestionado por software: escribe en la partición OTA inactiva, sincroniza los contadores de vida útil con el host antes de flashear y realiza una reversión automática si la nueva imagen flasheada no logra enumerarse en el bus USB. La actualización nunca requiere pulsar los botones físicos BOOT o RESET (AGENTS.md §1.5). <!-- src: /desktop/crates/opad-update/src/firmware.rs -->

## Secuencia de seguridad

Cada actualización de firmware sigue una estricta canalización de seguridad de cuatro pasos: <!-- src: /desktop/daemon/src/firmware_update.rs -->
1. **Consentimiento y verificación de reposo**: Las actualizaciones solo se inician cuando el jugador confirma explícitamente ("sí") y el teclado se encuentra en modo `IDLE` (nunca durante una partida o período de enfriamiento).
2. **Sincronización de contadores previa al flasheo**: El demonio sincroniza inmediatamente los contadores de vida útil del teclado con la base de datos local del host. Si surge algún fallo, el host puede restaurar la cifra exacta de contadores. <!-- src: /desktop/crates/opad-update/src/firmware.rs -->
3. **Comprobación criptográfica y de cabecera**: Se verifica el binario contra la firma SHA-256 en el manifiesto de versión firmado y se valida el identificador mágico del chip ESP32-S3 en la cabecera de la imagen. <!-- src: /desktop/daemon/src/firmware_update.rs -->
4. **Flasheo en la ranura opuesta**: El flasheador consulta la partición activa actualmente (`ota_0` u `ota_1`) y escribe la nueva imagen en la *otra* partición, actualizando `otadata` para un único arranque de prueba. <!-- src: /desktop/crates/opad-device/src/flash.rs -->

## Flasheo mediante opadctl (CLI)

La herramienta de línea de comandos `opadctl flash` es el mecanismo principal para instalar binarios de firmware compilados localmente o descargados: <!-- src: /desktop/cli/src/main.rs -->

### Actualización básica
```bash
opadctl flash firmware/build/opad-firmware.bin
```

### Qué ejecuta opadctl flash:
1. Contacta con `opad-daemon` a través de IPC (`IpcRequest::PrepareFlash`). El demonio cierra su puerto serie CDC y pausa el sondeo en segundo plano. <!-- src: /desktop/cli/src/main.rs -->
2. Envía el comando USB CDC para reiniciar el ESP32-S3 en modo de descarga ROM mediante una secuencia limpia de desconexión USB. <!-- src: /firmware/main/usb/usb_cdc.c#reboot_to_rom_download -->
3. Escribe el binario en la partición OTA inactiva (`OTA_0_OFFSET = 0x20000` u `OTA_1_OFFSET = 0x220000`) y registra el registro de prueba en `otadata`. <!-- src: /desktop/crates/opad-device/src/flash.rs#OTA_1_OFFSET -->
4. Reinicia el teclado a través de un comando de watchdog RTC.
5. Notifica a `opad-daemon` que el flasheo ha finalizado (`IpcRequest::FinishFlash`), lo que hace que vuelva a conectarse al firmware recién arrancado.

## Actualización mediante la GUI de escritorio

Cuando se publica una nueva versión oficial de firmware en el canal de actualización activo (`stable` o `beta`): <!-- src: /desktop/daemon/src/firmware_update.rs -->
1. Abre `opad-gui` y accede a **Ajustes > Actualizaciones**.
2. Si hay una actualización disponible, la interfaz mostrará la versión actual y la nueva versión de destino.
3. Haz clic en **Actualizar firmware**. Un cuadro de diálogo detallará las notas de la versión.
4. Haz clic en **Confirmar**: la aplicación de escritorio descarga la imagen, verifica el hash SHA-256, sincroniza los contadores, flashea el teclado y restablece la conexión en 10–15 segundos.

## Comportamiento de reversión (rollback)

Si una imagen de firmware recién flasheada falla durante la inicialización, se bloquea en un bucle de tareas o no logra enumerarse en el bus USB en 20 segundos, el arranque de prueba falla: <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->
- El teclado desencadena un reinicio de hardware automático.
- El bootloader del ESP32-S3 marca la ranura de prueba como no válida y arranca de nuevo en la ranura que funcionaba previamente.
- Tus ajustes, calibración y contadores de pulsaciones en NVS se mantienen intactos.
