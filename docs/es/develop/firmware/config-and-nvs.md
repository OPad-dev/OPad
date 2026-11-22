---
title: Subsistema de configuración y NVS del firmware
description: Referencia de implementación de espacios de nombres NVS del ESP32-S3, migración heredada osupad->opad, versionado de configuración y registros de calibración A/B con CRC32.
sidebar:
  order: 6
---

El estado persistente del teclado se almacena en la partición NVS del ESP32-S3 (`0x9000` en [/firmware/partitions.csv](/firmware/partitions.csv)). Todas las escrituras NVS están prohibidas durante `PLAYING` y `COOLDOWN` y se difieren a `IDLE`.

## Espacios de nombres NVS y migración (`nvs_ns.c`)

Definidos en [/firmware/main/config/nvs_ns.c](/firmware/main/config/nvs_ns.c):

| Enum (`nvs_ns_id_t`) | Nombre activo | Nombre heredado | Datos almacenados |
|---|---|---|---|
| `NVS_NS_COUNTERS` | `"opad"` | `"osupad"` | `generation`, `key1_cnt`, `key2_cnt`, `key3_cnt` ([/firmware/main/counters/counters.c](/firmware/main/counters/counters.c)) |
| `NVS_NS_CONFIG` | `"opad_cfg"` | `"osupad_cfg"` | `"cfg"` (`device_config_data_t` v7 incluyendo `owner_id[16]`), `"cfg_ext"` (`device_config_ext_t` v1), `"he_cfg"`, `"he_cfg3"` ([/firmware/main/config/device_config.c](/firmware/main/config/device_config.c)) |
| `NVS_NS_CAL` | `"opad_cal"` | `"osupad_cal"` | Calibración A/B Hall Effect V1 (`cal_a`, `cal_b`) ([/firmware/main/input/he_cal.c](/firmware/main/input/he_cal.c)) |
| `NVS_NS_CAL3` | `"opad_cal3"` | `"osupad_cal3"` | Calibración A/B Hall Effect V3 (`cal_a`/`cal_b` para Keys 1–2, `cal3_a`/`cal3_b` para Key 3) ([/firmware/main/input/he_cal.c](/firmware/main/input/he_cal.c)) |
| `NVS_NS_UI` | `"opad_ui"` | `"osupad_ui"` | Diseños personalizados de pantalla (`layout0`, `layout1`) ([/firmware/main/ui/ui_store.c](/firmware/main/ui/ui_store.c)) |

Al arrancar, `nvs_ns_migrate()` copia cada entrada de cada espacio de nombres heredado `osupad*` a su equivalente `opad*`, confirma todas las entradas y escribe al final el marcador `NVS_NS_MIGRATED_KEY` (`1`). Si algún paso falla o cae la alimentación a mitad de copia, se borra el espacio de nombres nuevo parcial y `nvs_ns_name()` continúa abriendo el espacio de nombres heredado para que los datos existentes nunca se pierdan.

## Compatibilidad de blobs de configuración (`device_config.h`, `config_validate.c`)

- `device_config_data_t` (versión `7`) admite cargar blobs heredados más cortos v1–v6 (`device_config_blob_layout_ok` en [/firmware/main/config/config_validate.c](/firmware/main/config/config_validate.c)) rellenando los campos nuevos con valores predeterminados.
- Los ajustes añadidos tras v7 (`key3_usage`, `game_keys`) viven en una clave separada `"cfg_ext"` (`device_config_ext_t`) para que revertir a firmware anterior nunca invalide `"cfg"`.
- Los ajustes Hall Effect viven en `"he_cfg"` (Keys 1–2) y `"he_cfg3"` (Key 3) como `he_config_record_t` (`HE_CONFIG_RECORD_MAGIC = 0x48454346` `"HECF"`, versión `1`, `flags` para `no_floor`, CRC-32 IEEE 802.3) ([/firmware/main/config/he_config_record.h](/firmware/main/config/he_config_record.h)).

## Registros atómicos de calibración A/B (`he_cal_record.h`, `he_cal.c`)

Implementados en [/firmware/main/input/he_cal_record.h](/firmware/main/input/he_cal_record.h) y [/firmware/main/input/he_cal.c](/firmware/main/input/he_cal.c):

- **Estructura (`he_cal_record_t`)**: `magic = 0x4F504341` (`"OPCA"`), `version = 1`, `seq` (`uint32_t` monotónico con comparación segura ante desbordamiento), `keys[2]` (`rest`, `full`, `valid`) y `crc32` IEEE 802.3.
- **Escrituras ping-pong A/B**: Cada guardado alterna entre la ranura A y la ranura B (`cal_a`/`cal_b`, y `cal3_a`/`cal3_b` para Key 3 en V3). Al arrancar se leen y validan por CRC ambas ranuras, eligiendo la ranura válida con mayor `seq`. Los registros ausentes o corruptos recurren a valores provisionales de fábrica (`valid = false`) sin borrar NVS.

## Volcado diferido

Cuando llegan actualizaciones de `SetConfig`, `SetLayout` o calibración durante `PLAYING` o `COOLDOWN`, el firmware actualiza la RAM de inmediato, marca las banderas de pendiente y espera a que `runtime_supervisor_task` ([/firmware/main/runtime/runtime.c](/firmware/main/runtime/runtime.c)) las vuelque al alcanzar `OPAD_STATE_IDLE`.
