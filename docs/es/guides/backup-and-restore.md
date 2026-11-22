---
title: Copias de seguridad y restauración
description: Exportación y restauración de copias de seguridad JSON de configuraciones de dispositivo, diseños de pantalla y contadores de pulsaciones mediante opadctl o la GUI de escritorio.
category: how-to
source_files:
  - /desktop/daemon/src/backup.rs
  - /desktop/cli/src/main.rs
  - /desktop/crates/opad-storage/src/lib.rs
---

OPad permite realizar copias de seguridad completas de su configuración, diseños de pantalla y estado de contadores serializados en un formato JSON portátil (`JsonBackup`). Las copias pueden exportarse manualmente o restaurarse en un nuevo teclado, y `opad-daemon` genera copias automáticas en segundo plano para garantizar que nunca se pierda el progreso si el hardware se daña o se sustituye. <!-- src: /desktop/daemon/src/backup.rs -->

## Copias de seguridad automatizadas

Para proteger los registros de pulsaciones sin intervención del usuario: <!-- src: /desktop/daemon/src/backup.rs -->
- `opad-daemon` guarda automáticamente una copia de seguridad en JSON tras cada sesión de juego activa.
- **Protección de la latencia**: De acuerdo con las reglas de latencia (Regla de producto 1), no se produce ninguna escritura en disco durante la partida. El demonio espera 20 segundos después de que finalice el enfriamiento posterior al mapa y el dispositivo vuelva a `IDLE` antes de escribir el archivo de copia de seguridad en el disco. <!-- src: /desktop/daemon/src/backup.rs -->
- Las copias de seguridad automáticas se guardan en el directorio de datos por usuario (`<data_dir>/backups/`). <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->

## Exportar una copia de seguridad

Puedes generar una copia de seguridad en cualquier momento utilizando `opadctl`: <!-- src: /desktop/cli/src/main.rs -->
```bash
opadctl backup export opad-backup.json
```
El archivo resultante contiene:
- `device_id`: Identificador de hardware del teclado respaldado.
- `counter_generation`: Número de generación de secuencia de los contadores.
- `lifetime_key1`, `lifetime_key2`, `lifetime_key3`: Total de pulsaciones acumuladas de cada tecla.
- `config`: Contenido completo de `DeviceConfig` (parámetros de rebote, asignación de pines, brillo del LED).
- `layouts`: Estructuras JSON personalizadas de widgets para los diseños de pantalla `idle` y `playing`.

### Exportar mediante la GUI
1. Abre `opad-gui` y accede a **Ajustes > Teclado**.
2. Haz clic en **Exportar copia de seguridad**.
3. Elige un archivo de destino y haz clic en **Guardar**.

## Previsualizar y restaurar una copia de seguridad

Antes de aplicar una copia de seguridad a un teclado conectado, `opadctl` ofrece un modo de previsualización que compara la copia entrante con el estado actual del teclado para detectar posibles riesgos de reversión de contadores: <!-- src: /desktop/cli/src/main.rs -->

### Previsualización
```bash
opadctl backup preview opad-backup.json
```
La previsualización comprueba:
- Si el `device_id` de la copia de seguridad coincide con el teclado conectado.
- Si restaurar la copia de seguridad provocaría una reducción en los contadores de vida útil respecto a las cifras actuales.
- Cualquier campo de diseño o configuración en conflicto.

### Restauración
Para aplicar la copia de seguridad al teclado y a la base de datos del escritorio: <!-- src: /desktop/cli/src/main.rs -->
```bash
opadctl backup import opad-backup.json --confirm
```
El demonio:
1. Valida la estructura del esquema JSON.
2. Incrementa el número `counter_generation` para que los nuevos contadores sustituyan limpiamente el estado del hardware existente. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->
3. Transmite `SetConfig` y `SetLayout` para actualizar la RAM del teclado.
4. Escribe los registros actualizados en SQLite y en la memoria NVS del teclado una vez en reposo.

### Restaurar mediante la GUI
1. En `opad-gui`, ve a **Ajustes > Teclado**.
2. Haz clic en **Importar copia de seguridad** y selecciona el archivo `.json`.
3. Revisa el cuadro de diálogo de diferencias que muestra los cambios en los contadores y las modificaciones de configuración.
4. Haz clic en **Confirmar importación** para aplicar los cambios.
