---
title: Copia de seguridad y restauración
description: Exporta los ajustes y los contadores de por vida de tu OPad a un archivo JSON, restáuralos viendo antes una vista previa y encuentra las copias automáticas que escribe el daemon tras cada sesión.
sidebar:
  order: 7
---

Una copia de seguridad es un archivo JSON con los ajustes de tu teclado y sus contadores de pulsaciones de por vida. Haz una a mano con `opadctl` o con la app, o usa las que el daemon escribe por su cuenta después de cada sesión de juego. Guarda una en un sitio seguro: es lo que te devuelve los recuentos si pierdes, sustituyes o borras un teclado. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup --> <!-- src: /desktop/daemon/src/backup.rs -->

## Qué necesitas

- La app de escritorio instalada, con el daemon en marcha. Para restaurar en el teclado, además tiene que estar conectado.
- Ningún mapa en curso. Las copias de seguridad están desactivadas durante un mapa y sus 5 segundos de cooldown. <!-- src: /desktop/gui/src/pages.rs#pad --> <!-- src: /desktop/daemon/src/runtime.rs#COOLDOWN_DURATION -->

## Qué contiene una copia de seguridad

| Parte | Contenido |
|---|---|
| `device` | el `device_id`, el `board_profile` (`waveshare_esp32s3_touch_lcd_2`) y la `counter_generation` del teclado |
| `stats` | `lifetime_key1`, `lifetime_key2` y `lifetime_key3` en un teclado de tres teclas cuando se conoce su recuento |
| `config` | códigos de tecla (`key1`, `key2` y, en un teclado de tres teclas, `key3` y `game_keys`), `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `key1_gpio`, `key2_gpio`, las cuatro acciones de deslizamiento con sus teclas y `result_animations` |
<!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup --> <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackupConfig -->

Los diseños de pantalla no están incluidos: expórtalos desde el Designer (consulta [Personalizar el diseño de la pantalla](customize-screen-layout.md#exportar-e-importar-un-diseño)). <!-- src: /desktop/gui/src/designer/mod.rs#export_layout -->

## Exportar una copia de seguridad

**Terminal:**

```bash
opadctl backup export opad-backup.json
```
<!-- src: /desktop/cli/src/main.rs#run -->

**App:**

1. Abre la página **Pad**.
2. En **Backup**, haz clic en **Export backup**.
3. Elige dónde guardar el archivo.
<!-- src: /desktop/gui/src/pages.rs#pad -->

## Restaurar una copia de seguridad

Al restaurar, siempre ves qué va a cambiar antes de que cambie nada.

**Terminal:**

```bash
opadctl backup import opad-backup.json
```

`opadctl` pide primero al daemon que compruebe el archivo y muestra una comparación: los recuentos actuales frente a los entrantes (Key 1, Key 2 y Key 3 si existe), si el `device_id` de la copia coincide con el teclado conectado y un aviso si algún recuento fuera a bajar. Responde `y` para aplicarla. Para saltarte la pregunta en un script, añade `--yes`: `opadctl backup import --yes opad-backup.json`. <!-- src: /desktop/cli/src/main.rs#run -->

**App:**

1. En la página **Pad**, en **Backup**, haz clic en **Import backup** y elige el archivo.
2. Revisa la vista previa: los recuentos actuales frente a los entrantes y si el teclado coincide. Si algún recuento fuera a bajar, marca **I understand this replaces my counters with lower values**.
3. Haz clic en **Apply Backup**.
<!-- src: /desktop/gui/src/main.rs#App -->

Al aplicarla, el daemon sube la generación de los contadores para que los recuentos restaurados ganen a las copias más antiguas, lo guarda todo en su base de datos y envía los ajustes y los recuentos al teclado, que los guarda en cuanto está en reposo. Se conserva tu dirección local de tosu. Consulta [Recuperación §5](../recovery.md#5-copia-de-seguridad-restauración-y-reinicio) para ver las reglas exactas. <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->

## Copias de seguridad automáticas

No tienes que hacer nada para estas: <!-- src: /desktop/daemon/src/backup.rs --> <!-- src: /desktop/daemon/src/runtime.rs#AUTO_BACKUP_DELAY -->

- **Cuándo:** 20 segundos después de que termine una sesión de juego y el teclado vuelva a estar en reposo. Nunca se escribe en disco durante el juego. Empezar otro mapa en esos 20 segundos cancela esa copia; la siguiente sesión hace una nueva.
- **Dónde:** `~/.local/share/opad/backups/` en Linux (o `$XDG_DATA_HOME/opad/backups/`), `%APPDATA%\opad\backups\` en Windows, con el nombre `opad-backup-<YYYYMMDDTHHMMSSZ>.json`. <!-- src: /desktop/daemon/src/backup.rs#backup_dir --> <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
- **Cuántas:** las 10 más recientes. Las más antiguas se borran; los archivos con cualquier otro nombre en esa carpeta no se tocan. <!-- src: /desktop/daemon/src/backup.rs#KEEP -->
- **La última:** `Last Backup:` en `opadctl status`. <!-- src: /desktop/cli/src/main.rs#run -->

Restáurala como cualquier otra copia de seguridad.

> [!WARNING]
> Desinstalar la app de escritorio borra esta carpeta, copias automáticas incluidas. Si quieres conservar una copia, cópiala antes a otro sitio. Consulta [Desinstalar](uninstall.md). <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

## Comprueba que ha funcionado

- Ejecuta `opadctl status` y revisa los recuentos de las teclas y los ajustes. <!-- src: /desktop/cli/src/main.rs#run -->
- En la app, **Counters** en la página **Pad** dice `The PC and the pad agree` cuando el PC y el teclado coinciden. <!-- src: /desktop/gui/src/pages.rs#pad -->

## Si no ha funcionado

- **Los botones están en gris o el daemon se niega:** sal del mapa y espera 5 segundos a que termine el cooldown. Una pregunta pendiente de "este teclado pertenece a otra instalación" o de "teclado nuevo" también bloquea la restauración hasta que la respondas. <!-- src: /desktop/gui/src/pages.rs#pad --> <!-- src: /desktop/daemon/src/ipc_handlers.rs#handle_ipc_request -->
- **Se rechaza el archivo:** una copia válida tiene `"format_version": 1`, `"board_profile": "waveshare_esp32s3_touch_lcd_2"`, ajustes dentro de sus rangos y un `exported_at` como mucho un día en el futuro. <!-- src: /desktop/crates/opad-model/src/lib.rs#JsonBackup -->
