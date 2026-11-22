---
title: Hall Effect y Rapid Trigger
description: Cómo lee OPad los interruptores magnéticos Hall Effect en hardware V1 y V3, ejecuta el motor Rapid Trigger de 3 estados, aplica pisos de ruido y almacena la calibración.
---

Los interruptores magnéticos Hall Effect (HE) sustituyen las láminas de contacto metálicas físicas por un imán en el vástago del interruptor y un sensor Hall lineal en la PCB. En lugar de un punto de contacto fijo, el firmware lee la intensidad magnética continua de cada tecla, permitiendo ajustar la profundidad de actuación y **Rapid Trigger (RT)**—soltando en cuanto el vástago empieza a subir y volviendo a pulsar en cuanto reanuda el movimiento descendente.

## Tuberías de sensado: ADC interno V1 frente a ADC SPI externo V3

OPad admite dos generaciones de hardware Hall Effect en [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c):

| Aspecto | Hall Effect V1 (2 teclas) | Hall Effect V3 (3 teclas) |
|---|---|---|
| Lectura de sensores | ADC1 interno del ESP32-S3 con DMA continuo (`HE_SOURCE_INTERNAL_ADC`) | ADC externo TI ADS7028 de 12 bits por SPI3 (`HE_SOURCE_SPI_ADC`) |
| Canales | Key 1 en `ADC_CHANNEL_9`, Key 2 en `ADC_CHANNEL_6` | Key 1 en `AIN5`, Key 2 en `AIN4`, Key 3 en `AIN3` (autosecuencia) |
| Cadencia de muestreo | `HE_SAMPLE_HZ = 64000` Hz, `HE_FRAME_CONVS = 16` -> tramas de **250 µs** (**4 kHz**) | `gptimer` hardware cada `HE_SPI_FRAME_US = 50` µs -> tramas de **20 kHz** |
| Bus / filtrado | Mediana de conversiones válidas por trama DMA de 250 µs dentro de la ISR en IRAM `on_frame` | SPI3 a 40 MHz con CRC por trama (bucle directo a registros `ads7028_read16_again`); 4 lecturas por trama (1 obsoleta + 3 teclas), descartando palabras con CRC erróneo |
| Decisión y despertar | La ISR `on_frame` ejecuta `he_rt_step` en IRAM en Core 0 y notifica a `he_task` solo en flancos de tecla | La tarea `he_spi` en Core 0 (`configMAX_PRIORITIES - 1`) ejecuta `he_rt_step` y publica eventos de tecla directamente vía `key_event_report` |
| Captura de calibración | Cada trama de 250 µs (4 kHz) | Diezmado 5:1 (`SPI_CAL_DECIMATE = 5`) para igualar la captura de calibración a 4 kHz |
| Almacenamiento NVS | `NVS_NS_CAL` (`opad_cal`), claves `cal_a`/`cal_b` | `NVS_NS_CAL3` (`opad_cal3`), claves `cal_a`/`cal_b` (Keys 1–2) + `cal3_a`/`cal3_b` (Key 3) |

Los detalles están en [/firmware/main/input/he_backend.c](/firmware/main/input/he_backend.c), [/firmware/main/input/he_spi_source.c](/firmware/main/input/he_spi_source.c), [/firmware/main/input/ads7028.c](/firmware/main/input/ads7028.c) y [/firmware/main/input/he_cal.c](/firmware/main/input/he_cal.c).

En V1, el controlador ADC continuo se configura con `ADC_CONTINUOUS_ISR_IRAM_SAFE` para que `on_frame` siga ejecutándose en RAM interna sin paradas de caché. En V3, `he_spi_source.c` adquiere SPI3 en exclusiva, realiza la primera transferencia mediante el driver y ejecuta las lecturas posteriores de 32 bits protegidas con CRC directamente sobre los registros hardware `GPSPI3` (`ads7028_read16_again`), evitando ~10 µs de sobrecarga del driver por palabra. Si un cable FFC se inserta al revés, `ads7028_carrier_pins(cable_reversed, ...)` intercambia automáticamente el mapeo de pines SPI.

## Motor de decisión Rapid Trigger (`he_rt.c`)

La máquina de estados de cada tecla en [/firmware/main/input/he_rt.c](/firmware/main/input/he_rt.c) (`he_rt_step`) tiene **tres** estados operativos (`he_rt_state_t`):

```text
                        raw >= actuation
            +----------------------------------------+
            |                                        v
   +---------------+   raw <= extreme - rt_release   +---------------+
   |  HE_RT_ARMED  | <-----------------------------  | HE_RT_PRESSED |
   +---------------+    (plain mode: raw <= release) +---------------+
            ^                                          |           ^
            |                                          | RT on:    | RT on:
            | raw <= region_exit                       | raw <=    | raw >=
            |                                          | extreme - | extreme +
            |                                          | rt_release| rt_press
            |                                          v           |
            +----------------------------------- +--------------------+
                                                 | HE_RT_RT_RELEASED  |
                                                 +--------------------+
```

### Transiciones de estado

1. **`HE_RT_ARMED`**: La tecla está suelta y esperando una pulsación inicial.
   - Si `k->need_release` está activo (tras arrancar, cambiar parámetros o sufrir un fallo de sensor), la tecla ignora pulsaciones hasta que `raw <= p->release` (o `raw <= p->region_exit` con Rapid Trigger activado).
   - Una vez armada y con `raw >= p->actuation`, la tecla pasa a `HE_RT_PRESSED`, emite `HE_RT_DOWN` e inicializa `k->extreme` con `cap_peak(raw, p->peak_cap)`.
2. **`HE_RT_PRESSED`**: La tecla está mantenida pulsada.
   - Sin Rapid Trigger (`!p->rapid_trigger`): la tecla sigue pulsada hasta que `raw <= p->release`, volviendo entonces a `HE_RT_ARMED` y emitiendo `HE_RT_UP`.
   - Con Rapid Trigger:
     - `k->extreme` sigue la lectura cruda más profunda alcanzada, limitada a `p->peak_cap` (lo que implementa la **BOTTOM DEAD ZONE** opcional para que apretar con más fuerza contra el tope de la carcasa no desplace el punto de liberación).
     - Si `raw <= p->region_exit`, la tecla sale de la zona Rapid Trigger, vuelve a `HE_RT_ARMED` y emite `HE_RT_UP`.
     - En caso contrario, si `raw <= k->extreme - p->rt_release`, la tecla pasa a `HE_RT_RT_RELEASED`, emite `HE_RT_UP` y fija `k->extreme = raw` para seguir el punto más superficial alcanzado.
3. **`HE_RT_RT_RELEASED`**: La tecla se soltó a mitad de recorrido por Rapid Trigger y está lista para volver a pulsar de inmediato.
   - `k->extreme` sigue la lectura más baja (más suelta) observada al subir.
   - Si `raw <= p->region_exit`, termina Rapid Trigger y la tecla regresa a `HE_RT_ARMED` (requiriendo cruzar de nuevo `p->actuation` la próxima vez).
   - Si `raw >= k->extreme + p->rt_press`, la tecla vuelve a actuar inmediatamente (`HE_RT_DOWN`), retorna a `HE_RT_PRESSED` y restablece `k->extreme = cap_peak(raw, p->peak_cap)`.

La diferencia entre Rapid Trigger **Normal** y **Continuous** es simplemente `p->region_exit`: en modo Normal se sitúa en el umbral de actuación (Rapid Trigger termina cuando la tecla sube de nuevo por encima de la actuación), mientras que en modo Continuo se sitúa cerca de arriba en `top_reset` para que Rapid Trigger permanezca activo casi hasta la posición de reposo.

## Unidades de recorrido, pisos de ruido y calibración

Según [/docs/specs/v2-hall-effect-decisions.md](/docs/specs/v2-hall-effect-decisions.md) (D3 y D7) y [/firmware/main/input/he_params.h](/firmware/main/input/he_params.h):

- **Porcentaje del rango magnético (`HE_PCT_FULL = 1000`)**: Los ajustes se guardan en incrementos de `0.1 %` de `full - rest` (conteos magnéticos, no milímetros), porque la fuerza del imán y la respuesta del sensor varían de forma no lineal entre interruptores.
- **Pisos de ruido (`he_params_build`)**: Los umbrales de movimiento (`rt_press`, `rt_release`, `release_hyst`) y el margen de reinicio superior se limitan por encima de los pisos de ruido en conteos crudos configurados (`he_limits_t`), y `he_params_build` devuelve banderas explícitas `HE_PARAMS_*_RAISED` siempre que sube un ajuste—nunca en silencio. El usuario puede desactivarlo por tecla mediante **Allow below the noise floor** (`no_floor`, bajando los pisos a 1 conteo).
- **Calibración de cinco pulsaciones (`he_cal_capture.h`)**: La posición de reposo se promedia sobre 64 tramas (16 ms a 4 kHz tras un asentamiento de 64 tramas) y se comprueba su dispersión y límites absolutos. El fondo de recorrido registra 5 pulsaciones completas por tecla y toma el pico **mediano** para que una pulsación aislada demasiado fuerte no distorsione el rango; también comprueba si la tecla soltó por debajo de la posición de reposo medida (`HE_CAL_ERR_REST_PRESSED`).
- **Persistencia NVS A/B (`he_cal.c`)**: La calibración alterna entre `cal_a`/`cal_b` (y `cal3_a`/`cal3_b` para Key 3 en V3) con números de secuencia y validación CRC32. V1 (`opad_cal`) y V3 (`opad_cal3`) usan espacios de nombres NVS separados para que la calibración de una generación de hardware nunca impulse a la otra. Las escrituras NVS se difieren al jugar (`PLAYING` / `COOLDOWN`) y se vuelcan en `IDLE`.

## Seguridad, bloqueo sin calibración y recuperación ante bloqueo de ADC

- **Bloqueo sin calibración (D7)**: Hasta que cada tecla activa cuente con una calibración válida (`he_cal_is_locked_decision`), `he_cal_suppress_input` suprime toda salida de teclas, los deslizamientos táctiles quedan desactivados y la pantalla se bloquea en la vista de calibración ([/firmware/main/ui/he_cal_ui.c](/firmware/main/ui/he_cal_ui.c), [/firmware/main/input/he_cal_runner.c](/firmware/main/input/he_cal_runner.c)).
- **Watchdog de parada de ADC (`he_health`)**: Si una tecla deja de producir conversiones válidas durante `HE_STALL_US`, `he_backend.c` fuerza la liberación de la tecla (`he_rt_fault`) para que un fallo de sensor o cable nunca deje una tecla atascada.
- **Recuperación segura tras fallo**: `he_rt_fault` activa `need_release = true`, de modo que cuando vuelven las lecturas válidas debe observarse la tecla en posición de reposo antes de emitir cualquier nueva pulsación.
- **Pausa para sondeo de módulo**: Cuando el monitor de módulos V3 mide la tensión de ID del módulo, `he_backend_run_paused` pausa limpiamente la fuente ADC, lee la tensión de ID, libera la unidad oneshot y reanuda el flujo sin dejar teclas atascadas.
