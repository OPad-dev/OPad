---
title: Actualizar el firmware
description: Actualiza el firmware de OPad por USB desde la app u opadctl. El firmware nuevo va a la otra ranura y se revierte solo si no funciona.
sidebar:
  order: 1
---

El firmware de OPad se actualiza por el mismo cable USB con el que juegas. Nunca pulsas los botones BOOT o RESET de la placa, y una actualización fallida no puede dejar el teclado inservible: el firmware nuevo va a la otra de las dos ranuras de firmware del teclado y, si no arranca como un dispositivo USB que funcione, el teclado vuelve solo al antiguo. <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update --> <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->

La app y el teclado se actualizan por separado. Puedes actualizar el firmware sin actualizar la app, y al revés. <!-- src: /AGENTS.md#1. Product rules -->

## Antes de empezar

- Conecta el teclado directamente al PC y no a través de un hub que puedas golpear. <!-- src: /desktop/crates/opad-update/src/firmware.rs#consent_text -->
- Ten la app de escritorio instalada y en marcha. `opadctl pad flash` también funciona sin el daemon, pero entonces no puede conservar el firmware antiguo como respaldo (consulta [más abajo](#flashear-un-archivo-de-firmware-opadctl-flash)). <!-- src: /desktop/cli/src/main.rs#run -->
- El teclado deja de funcionar como teclado durante unos 30 segundos mientras se actualiza, así que hazlo fuera de un mapa. De todas formas, OPad se niega a empezar una actualización durante un mapa. <!-- src: /desktop/crates/opad-update/src/firmware.rs#TYPICAL_OUTAGE_SECONDS --> <!-- src: /desktop/crates/opad-update/src/firmware.rs#blockers -->

Se conservan tus contadores de por vida, ajustes, diseños, calibración Hall Effect y registro del propietario: una actualización solo escribe la ranura del firmware. <!-- src: /desktop/daemon/src/firmware_update.rs --> <!-- src: /firmware/partitions.csv -->

## Actualizar desde la app

1. Abre **Settings** → **Updates**. (Cuando hay un firmware más nuevo, **Update to …** en la página **Pad** te lleva ahí.) <!-- src: /desktop/gui/src/settings.rs#updates --> <!-- src: /desktop/gui/src/pages.rs#pad -->
2. La fila **Pad firmware** muestra la versión instalada, la ranura desde la que se ejecuta y la versión más reciente si la hay, con sus notas de la versión. Todo lo que impide ahora mismo una actualización (sin teclado, un mapa en curso, una pregunta de propiedad abierta) aparece con un `⚠`. <!-- src: /desktop/gui/src/settings.rs#firmware_row -->
3. Haz clic en **Update now**. Solo está habilitado cuando hay una actualización disponible y nada lo impide. <!-- src: /desktop/gui/src/settings.rs#firmware_row -->
4. Lee el diálogo **Confirm Firmware Update** y haz clic en **Confirm & Flash Pad**. <!-- src: /desktop/gui/src/main.rs#App -->
5. Deja el teclado conectado hasta que aparezca **✓ Firmware Update Complete** y luego haz clic en **Close**. <!-- src: /desktop/gui/src/main.rs#App -->

### Actualizaciones automáticas

**Install firmware updates automatically** está activado de forma predeterminada. Con él activado, el daemon instala por su cuenta una nueva versión del firmware cuando el teclado lleva 30 minutos sin usarse: ninguna pulsación en 30 minutos y, salvo que hayas configurado la pantalla para no suspenderse nunca, la pantalla suspendida. Nunca actualiza durante un mapa. Si falla una actualización automática de una versión, no vuelve a intentar esa versión durante un día. Desmarca la casilla para actualizar solo cuando hagas clic en **Update now**. <!-- src: /desktop/daemon/src/firmware_update.rs#AUTO_IDLE --> <!-- src: /desktop/daemon/src/firmware_update.rs#auto_wait --> <!-- src: /desktop/daemon/src/main.rs --> <!-- src: /desktop/daemon/src/updater.rs#read_flag -->

## Actualizar desde un terminal (`opadctl update install firmware`)

Esto instala la misma versión que la app, de la misma manera:

```sh
opadctl update install firmware
```

Muestra las versiones instalada y disponible, las notas de la versión y lo que hará la actualización, y luego pregunta `Flash the pad now? [y/N]:`. Responde `y`. En un script, pasa `--yes` para dar esa respuesta de antemano. <!-- src: /desktop/cli/src/main.rs#run -->

## Flashear un archivo de firmware (`opadctl pad flash`)

Para instalar un archivo de firmware que hayas compilado o descargado tú:

```sh
opadctl pad flash firmware/build/opad-firmware.bin
```

`opadctl pad flash`: <!-- src: /desktop/cli/src/main.rs#run -->

1. comprueba que el archivo es una imagen de firmware para ESP32-S3; <!-- src: /desktop/crates/opad-device/src/flash.rs#check_esp32s3_image_file -->
2. pregunta al daemon desde qué ranura se ejecuta el teclado y elige la otra (`ota_0` en `0x20000` u `ota_1` en `0x220000`); <!-- src: /desktop/crates/opad-device/src/flash.rs#plan_app_update -->
3. pide al daemon que libere el puerto serie del teclado y reinicia el teclado en modo de descarga por USB; <!-- src: /desktop/cli/src/main.rs#prepare_flash --> <!-- src: /desktop/crates/opad-device/src/flash.rs#enter_bootloader -->
4. escribe la imagen con el `espflash` incluido y luego reinicia el teclado con ella; <!-- src: /desktop/crates/opad-device/src/flash.rs#write_images --> <!-- src: /desktop/crates/opad-device/src/flash.rs#reset_to_app -->
5. espera hasta 45 segundos a que vuelva el teclado y te dice si se ejecuta desde la ranura nueva o si se ha revertido. <!-- src: /desktop/cli/src/main.rs#report_slot_after_flash -->

> [!WARNING]
> Sin el daemon (o con un firmware demasiado antiguo para comunicar su ranura), `opadctl pad flash` no puede saber qué ranura se está ejecutando. Entonces escribe `ota_0`, deja en blanco la selección de arranque para que arranque `ota_0`, y muestra `Running slot unknown: writing ota_0 and making it the slot that boots, with no fallback`. Si ese flasheo se interrumpe, recupera el teclado con `opadctl pad recover` ([Recuperación §7.7](../recovery.md#77-un-teclado-que-no-aparece-como-opad-opadctl-recover)). <!-- src: /desktop/cli/src/main.rs#run -->

Opciones: <!-- src: /desktop/cli/src/main.rs#run -->

- `--port <port>`: el puerto del teclado, cuando no quieres la detección automática (o hay varios teclados conectados).
- `--full`: un flasheo de recuperación. Pásale una carpeta (una carpeta de compilación de ESP-IDF o los archivos `.bin` de una versión publicada del firmware) y también escribe el bootloader, la tabla de particiones y los datos OTA. Escribe `ota_0` en su sitio, así que úsalo solo para recuperar; consulta [Recuperación §7.4](../recovery.md#74-borra-y-luego-flashea).

Los paquetes incluyen `espflash`; `opadctl` lo busca junto a sí mismo, luego en `<prefix>/lib/opad/bin/` y después en tu `PATH`. <!-- src: /desktop/crates/opad-device/src/flash.rs#resolve_espflash -->

## Qué comprueba por ti una actualización desde la app

Antes de escribir nada, el daemon comprueba que el teclado está conectado y en reposo, que se ejecuta desde una de sus dos ranuras, que no tiene abierta ninguna pregunta de propiedad o de sustitución y que esta versión de la app es lo bastante reciente para el firmware. Después sincroniza tus contadores con el PC, descarga el firmware, verifica la firma de la versión y el SHA-256 del archivo, comprueba que la imagen es para el ESP32-S3 y comprueba una vez más que no ha empezado ningún mapa durante la descarga. Tras flashear, espera hasta 45 segundos a que el teclado vuelva en la ranura nueva con la versión nueva. <!-- src: /desktop/daemon/src/firmware_update.rs#install --> <!-- src: /desktop/crates/opad-update/src/firmware.rs#blockers --> <!-- src: /desktop/crates/opad-update/src/verify.rs#trusted_keys --> <!-- src: /desktop/daemon/src/firmware_update.rs#still_safe_to_flash --> <!-- src: /desktop/daemon/src/firmware_update.rs#RECONNECT_TIMEOUT -->

## Reversión automática

El firmware nuevo arranca a prueba. Solo se confirma a sí mismo cuando tu PC ha configurado su conexión USB, lo que demuestra que el teclado vuelve a funcionar como teclado. <!-- src: /firmware/main/app_main.c#confirm_image_once_usb_works -->

- **Funciona:** se conserva la ranura nueva.
- **El PC nunca lo configura:** tras 20 segundos conectado a un PC sin que eso ocurra, el teclado se reinicia y vuelve al firmware anterior. <!-- src: /firmware/main/app_main.c#IMAGE_CONFIRM_TIMEOUT_US -->
- **Falla antes de confirmarse:** el teclado vuelve al firmware anterior de la misma manera.
- **En un cargador, sin PC:** no se decide nada; la prueba espera hasta que conectes el teclado a un ordenador.

## Comprueba que ha funcionado

```sh
opadctl status
```

Busca: <!-- src: /desktop/cli/src/main.rs#run -->

- `ESP32 Device:     Connected`
- `Firmware Version:` la versión que has instalado
- `Running Slot:` `ota_0` u `ota_1`

En la app, la página **Pad** muestra **Firmware**, y **Running slot** dentro de **Details**. <!-- src: /desktop/gui/src/pages.rs#pad -->

## Si no ha funcionado

- **El teclado ha vuelto al firmware antiguo:** sigue funcionando como antes. Consulta [Recuperación §7.6](../recovery.md#76-cuando-se-interrumpe-el-flasheo).
- **El teclado ya no aparece como OPad (`303a:8415`):** ejecuta `opadctl pad recover` y vuelve a conectar el teclado, como en [Recuperación §7.7](../recovery.md#77-un-teclado-que-no-aparece-como-opad-opadctl-recover).
- **El puerto serie está ocupado o se deniega el permiso:** consulta [Solución de problemas §3](../troubleshooting.md#3-linux-permisos-usb-y-el-puerto-serie).
