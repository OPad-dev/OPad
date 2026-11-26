---
title: Subsistema de configuración y NVS del firmware
description: Referencia de implementación de los namespaces NVS del ESP32-S3, la migración legacy osupad->opad, el versionado de la configuración y los registros de calibración A/B con CRC32.
sidebar:
  order: 6
---

El estado persistente del pad se guarda en la partición NVS del ESP32-S3 (`nvs` en `0x9000`, 24 KB, en [/firmware/partitions.csv](https://github.com/OPad-dev/OPad/blob/main/firmware/partitions.csv)). Todos los commits de NVS están prohibidos durante `PLAYING` y `COOLDOWN` y se difieren a `IDLE` ([máquina de estados de ejecución](overview.md#máquina-de-estados-de-ejecución-runtimec)). Qué significa cada ajuste para un usuario está en [Ajustes del dispositivo](../../reference/device-settings.md); cómo sobreviven los ajustes a una reversión de la app o del firmware, en [Versiones de la app y del pad](../../explanation/app-and-pad-versions.md).

## Namespaces NVS y migración (`nvs_ns.c`)

Definidos en [/firmware/main/config/nvs_ns.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/nvs_ns.c):

| Enum (`nvs_ns_id_t`) | Nombre activo | Nombre legacy | Datos guardados |
|---|---|---|---|
| `NVS_NS_COUNTERS` | `"opad"` | `"osupad"` | `generation`, `key1_cnt`, `key2_cnt`, `key3_cnt` ([/firmware/main/counters/counters.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counters.c)) |
| `NVS_NS_CONFIG` | `"opad_cfg"` | `"osupad_cfg"` | `"cfg"` (`device_config_data_t` v7, incluido `owner_id[16]`), `"cfg_ext"` (`device_config_ext_t` v1), `"he_cfg"`, `"he_cfg3"` ([/firmware/main/config/device_config.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/device_config.c)) |
| `NVS_NS_CAL` | `"opad_cal"` | `"osupad_cal"` | Calibración A/B Hall Effect V1 (`cal_a`, `cal_b`) ([/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c)) |
| `NVS_NS_CAL3` | `"opad_cal3"` | `"osupad_cal3"` | Calibración A/B Hall Effect V3 (`cal_a`/`cal_b` para las Keys 1–2, `cal3_a`/`cal3_b` para la Key 3) ([/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c)) |
| `NVS_NS_UI` | `"opad_ui"` | `"osupad_ui"` | Layouts de pantalla personalizados (`layout0`, `layout1`) ([/firmware/main/ui/ui_store.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/ui_store.c)) |

Al arrancar, `nvs_ns_migrate()` copia cada entrada de cada namespace legacy `osupad*` a su namespace `opad*`, hace commit de todas y escribe al final la marca `NVS_NS_MIGRATED_KEY` (`"_migrated"` = `1`). Si falla algún paso o se va la alimentación a mitad de la copia, se borra el namespace nuevo a medias, `nvs_ns_name()` sigue devolviendo el nombre legacy y el siguiente arranque vuelve a empezar la copia desde cero. El namespace legacy solo se lee y nunca se vuelve a escribir, así que un firmware anterior al cambio de nombre sigue encontrando sus datos tras una reversión. Probado en el host en [/firmware/test/host/test_nvs_ns.c](https://github.com/OPad-dev/OPad/blob/main/firmware/test/host/test_nvs_ns.c).

## Compatibilidad de los blobs de configuración (`device_config.h`, `config_validate.c`)

- `device_config_data_t` (versión `7`) admite cargar blobs legacy más cortos, de v1 a v6 (`device_config_blob_layout_ok` en [/firmware/main/config/config_validate.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/config_validate.c)), y rellena con valores por defecto los campos más nuevos.
- Los ajustes añadidos después de v7 (`key3_usage`, `game_keys`) viven en una clave aparte, `"cfg_ext"` (`device_config_ext_t`), para que volver a un firmware anterior nunca invalide `"cfg"`.
- Los ajustes Hall Effect viven en `"he_cfg"` (Keys 1–2) y `"he_cfg3"` (Key 3) como `he_config_record_t` (`HE_CONFIG_RECORD_MAGIC = 0x48454346` `"HECF"`, versión `1`, `flags` para `no_floor`, CRC-32 IEEE 802.3) ([/firmware/main/config/he_config_record.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/config/he_config_record.h)).

## Registros de calibración Hall Effect

Registros A/B atómicos (`he_cal_record.h`, `he_cal.c`).

Implementados en [/firmware/main/input/he_cal_record.h](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal_record.h) y [/firmware/main/input/he_cal.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/he_cal.c):

- **Estructura (`he_cal_record_t`)**: `magic = 0x4F504341` (`"OPCA"`), `version = 1`, `seq` (`uint32_t` monótono con comparación segura frente al desbordamiento), `keys[2]` (`rest`, `full`, `valid`) y `crc32` IEEE 802.3.
- **Valores de fábrica**: `rest = 2086`, `full = 2536`, usados con `valid = false` hasta que se guarda una calibración; un pad con alguna tecla no válida está bloqueado ([entrada](input.md#calibración-y-el-bloqueo-decisión-d7)).
- **Escrituras A/B alternas**: cada guardado alterna entre la ranura A y la ranura B (`cal_a`/`cal_b`, y `cal3_a`/`cal3_b` para la Key 3 en V3). Al arrancar se leen las dos ranuras y se valida su CRC, eligiendo la ranura válida con el `seq` más reciente. Si faltan registros o están corruptos, se recurre a los valores de fábrica provisionales (`valid = false`) sin borrar la NVS.

## Escritura diferida

Cuando llegan `SetConfig`, `SetLayout` o actualizaciones de calibración durante `PLAYING` o `COOLDOWN`, el firmware actualiza la RAM al instante, marca los datos como pendientes y espera a que `runtime_supervisor_task` ([/firmware/main/runtime/runtime.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/runtime/runtime.c)) (la tarea llamada `runtime_task`) los escriba cuando se llegue a `OPAD_STATE_IDLE`: `device_config_flush()`, `ui_store_flush_dirty()` ([UI](ui.md#layouts-y-persistencia-diferida-ui_storec)) y `he_cal_flush()`. Los contadores siguen sus propias reglas de checkpoint ([contadores](counters.md)).
