---
title: Copia de seguridad y restauración
description: Cómo funcionan las copias de seguridad JSON automáticas y cómo exportar, previsualizar y restaurar la configuración y los contadores de por vida de OPad con opadctl u opad-gui.
sidebar:
  order: 7
---

OPad exporta la configuración del dispositivo y los contadores de pulsaciones de por vida como archivos JSON portables (`JsonBackup`, `format_version: 1`). Puedes exportar y restaurar copias de seguridad manualmente mediante `opadctl` u `opad-gui`, y `opad-daemon` también escribe copias automáticas tras las sesiones de juego para que los contadores sobrevivan a daños o reemplazos de hardware. <!-- src: /desktop/daemon/src/backup.rs --> <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->

## Qué necesitas

- `opad-daemon` en ejecución (y tu OPad conectado por USB al restaurar sobre el hardware).
- El teclado debe estar en reposo (no en partida activa ni en enfriamiento post-mapa); las operaciones de copia de seguridad se deshabilitan durante `Playing` y `Cooldown`. <!-- src: /desktop/gui/src/pages.rs#device -->

## Copias de seguridad automáticas

- Cuando termina una sesión de juego (`COOLDOWN` -> `SYNC` -> `IDLE`), `opad-daemon` espera **20 segundos** en `IDLE` antes de escribir una copia automática para que la E/S de disco nunca toque la ruta de juego (iniciar otro mapa cancela el temporizador pendiente). <!-- src: /desktop/daemon/src/backup.rs -->
- Los archivos se escriben atómicamente (`write_atomic`) en `<data_dir>/backups/opad-backup-<YYYYMMDDTHHMMSSZ>.json` (consulta [Archivos y rutas](../reference/files-and-paths.md)). <!-- src: /desktop/daemon/src/backup.rs#backup_dir -->
- El daemon conserva los **10** archivos de copia automática más recientes (`KEEP = 10`) y elimina los más antiguos (dejando intactos los archivos nombrados manualmente). <!-- src: /desktop/daemon/src/backup.rs#KEEP -->

## Qué contiene una copia de seguridad

Un archivo `JsonBackup` registra: <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
- `device`: `device_id`, `board_profile` (`waveshare_esp32s3_touch_lcd_2`) y `counter_generation`.
- `stats`: `lifetime_key1`, `lifetime_key2` y `lifetime_key3` opcional (incluido en teclados de 3 teclas cuando se conoce).
- `config`: `key1`, `key2`, `key3` y `game_keys` opcionales (teclados de 3 teclas), `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `key1_gpio`, `key2_gpio`, acciones/teclas/modificadores de deslizamiento táctil y `result_animations`.

(Los diseños personalizados de pantalla se exportan e importan por separado dentro de la página **Designer** en `opad-gui`.) <!-- src: /desktop/gui/src/designer/mod.rs -->

## Exportar una copia de seguridad

### Mediante `opadctl`
```bash
opadctl export opad-backup.json
```
<!-- src: /desktop/cli/src/main.rs#Commands::Export -->

### Mediante `opad-gui`
1. Abre `opad-gui` y haz clic en **Device** en la barra lateral. <!-- src: /desktop/gui/src/pages.rs#device -->
2. Desplázate hasta la tarjeta **Backup & Restore** y haz clic en **Export backup**. <!-- src: /desktop/gui/src/pages.rs#device -->
3. Elige dónde guardar el archivo `.json`.

## Previsualizar y restaurar una copia de seguridad

### Mediante `opadctl`
Ejecuta `opadctl import` con el archivo de copia de seguridad: <!-- src: /desktop/cli/src/main.rs#Commands::Import -->
```bash
opadctl import opad-backup.json
```
Antes de modificar nada, `opadctl` envía `IpcRequest::PreviewImport` para validar el archivo e imprime una comparación que muestra:
- Conteos de por vida actuales frente a entrantes (`Key 1`, `Key 2` y `Key 3` si está presente).
- Si `device_id` coincide con el teclado conectado.
- Advertencias si la importación hiciera retroceder los contadores.

Escribe `y` en el aviso de confirmación para aplicarla (o pasa `--yes` para omitir la pregunta: `opadctl import --yes opad-backup.json`). <!-- src: /desktop/cli/src/main.rs#Commands::Import -->

### Mediante `opad-gui`
1. Abre `opad-gui` y haz clic en **Device** en la barra lateral. <!-- src: /desktop/gui/src/pages.rs#device -->
2. En la tarjeta **Backup & Restore**, haz clic en **Import backup** y elige tu archivo `.json`.
3. Revisa el diálogo de previsualización (contadores actuales frente a entrantes, comprobación de ID de dispositivo y casilla de advertencia de retroceso si aplica) y confirma la importación. <!-- src: /desktop/gui/src/main.rs#ImportModalState -->

Al confirmar, el daemon incrementa `counter_generation`, actualiza SQLite, envía la configuración y los contadores al teclado y los guarda en NVS al entrar en reposo. <!-- src: /desktop/daemon/src/ipc_handlers.rs -->

## Cómo comprobarlo

- Ejecuta `opadctl status` (o revisa la página **Device** en `opad-gui`) y verifica que **Counter Reconciliation State** muestre **✓ SYNCHRONIZED** con los valores esperados de `lifetime_key1`, `lifetime_key2` y configuración. <!-- src: /desktop/gui/src/pages.rs#device -->

## Qué hacer si no funcionó

- **Los botones están deshabilitados o el daemon rechaza la importación**: Sal del mapa de osu! actual y espera 5 segundos a que termine el enfriamiento post-mapa para que el teclado vuelva a `Idle`. <!-- src: /desktop/gui/src/pages.rs#device -->
- **Error de validación al importar**: Comprueba que el archivo JSON tenga `"format_version": 1`, `"board_profile": "waveshare_esp32s3_touch_lcd_2"` y rangos de configuración válidos. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
