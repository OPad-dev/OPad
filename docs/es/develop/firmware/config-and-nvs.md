---
title: Subsistema de Configuración y NVS del Firmware
description: Referencia de implementación para la gestión de Non-Volatile Storage (NVS), migración de espacios de nombres, registros atómicos A/B y verificación CRC32.
---

El subsistema de configuración y almacenamiento gestiona el estado persistente del pad dentro de la partición flash de Non-Volatile Storage (NVS) del ESP32-S3. Aísla las operaciones de borrado y escritura en flash del juego activo, gestiona la migración de espacios de nombres y garantiza actualizaciones atómicas de registros mediante ranuras A/B protegidas por suma de verificación. <!-- src: /firmware/main/config/device_config.c -->

## NVS Namespaces & Migration (nvs_ns.c)

El almacenamiento está dividido en cinco espacios de nombres lógicos distintos (`nvs_ns.c`): <!-- src: /firmware/main/config/nvs_ns.c -->

| Constante de Espacio de Nombres | Nombre Activo | Nombre Legacy | Contenido |
|---|---|---|---|
| `NVS_NS_COUNTERS` | `"opad"` | `"osupad"` | Contadores de pulsaciones monotónicos de por vida, generación de contadores, identidad de propietario. <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_CONFIG` | `"opad_cfg"` | `"osupad_cfg"` | Ventana de antirrebote, asignación de pines de teclas, brillo de pantalla, enlaces de deslizamiento. <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_CAL` | `"opad_cal"` | `"osupad_cal"` | Registros de calibración de Hall Effect para Key 1 y Key 2. <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_CAL3` | `"opad_cal3"` | `"osupad_cal3"` | Registros de calibración de Hall Effect para Key 3 (módulo V3). <!-- src: /firmware/main/config/nvs_ns.c --> |
| `NVS_NS_UI` | `"opad_ui"` | `"osupad_ui"` | Diseños de pantalla personalizados de widgets serializados (`idle` y `playing`). <!-- src: /firmware/main/config/nvs_ns.c --> |

Para garantizar la compatibilidad hacia atrás (Regla de Producto 3), el resolutor de espacios de nombres comprueba la existencia de nombres de espacios de nombres legacy antes de abrir los activos; los pads existentes leen sus registros antiguos de forma transparente sin perder datos. <!-- src: /firmware/main/config/nvs_ns.c -->

## Atomic A/B Record Architecture (he_cal_record.c)

Las escrituras en flash conllevan riesgo de corrupción si la alimentación se interrumpe a mitad de la transacción. Los registros críticos (como la calibración y configuración de Hall Effect) emplean un esquema de almacenamiento alterno A/B ping-pong: <!-- src: /firmware/main/input/he_cal_record.c -->

- **Estructura del Registro (`he_cal_record_t`)**:
  - `magic`: Magic de validación de 32 bits (`HE_CAL_MAGIC = 0x4845434C`). <!-- src: /firmware/main/input/he_cal_record.c -->
  - `version`: Revisión del formato (`HE_CAL_VERSION = 1`).
  - `seq`: Número de secuencia de 32 bits en incremento monotónico.
  - `keys[2]`: Estructuras de calibración por tecla (reposo, fondo, recorrido).
  - `crc32`: Suma de comprobación estándar IEEE 802.3 CRC32 que cubre todo el registro. <!-- src: /firmware/main/input/he_cal_record.c#he_cal_crc32 -->
- **Dos Ranuras (`cal_a` y `cal_b`)**:
  - Al arrancar, se leen ambas ranuras y se validan contra sus sumas de comprobación CRC32.
  - Si ambas son válidas, se carga la ranura con el número de secuencia (`seq`) más alto.
  - Al escribir actualizaciones, la ranura inactiva se escribe y se confirma primero antes de invalidar o reemplazar la ranura previa. Una escritura interrumpida deja intacto el registro válido anterior.

## Runtime Staging & Flash Invariant

Los ciclos de borrado en flash tardan entre 20 ms y 100 ms, deteniendo la ejecución de la caché de CPU en ambos núcleos Xtensa: <!-- src: /firmware/main/config/device_config.c -->
- Cuando el host envía `SetConfig` o `SetLayout`, los ajustes se aplican inmediatamente a las variables activas en RAM (`s_config`).
- Si el estado del dispositivo es `OPAD_STATE_PLAYING` u `OPAD_STATE_COOLDOWN`, las escrituras en flash se posponen (`s_dirty = true`).
- El supervisor de runtime vuelca los registros de configuración pendientes a NVS únicamente tras el regreso del pad a `OPAD_STATE_IDLE`.
