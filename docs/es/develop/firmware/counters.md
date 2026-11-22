---
title: Subsistema de Contadores del Firmware
description: Referencia de implementación para el seguimiento monotónico de pulsaciones, aplicación atómica de deltas, validación de generación y puntos de control perezosos en flash.
---

El subsistema de contadores rastrea las actuaciones acumuladas de por vida en Key 1, Key 2 y Key 3. Aplica una progresión monotónica estricta, evita el desgaste de la memoria flash mediante puntos de control perezosos por inactividad y asegura que las pulsaciones efectuadas durante la sincronización USB nunca se pierdan. <!-- src: /firmware/main/counters/counters.c -->

## Storage & RAM State (counters.c)

- **Variables en RAM**: Los contadores en vivo se mantienen en enteros sin signo atómicos y rápidos de 64 bits en Core 0: <!-- src: /firmware/main/counters/counters.c -->
  - `s_generation`: Número de generación de contadores (comienza en 1).
  - `s_last_saved_key1`, `s_last_saved_key2`, `s_last_saved_key3`: Valores base guardados en puntos de control.
- **Bloqueo de Serialización (`s_lock`)**: Un semáforo binario de FreeRTOS serializa las operaciones de punto de control, sincronización y restablecimiento entre el supervisor de runtime de Core 1 y la tarea de protocolo. Las ISR de teclas de Core 0 nunca toman este bloqueo. <!-- src: /firmware/main/counters/counters.c -->

## Monotonic Sync Validation (counter_sync_rules.c)

Cuando `opad-daemon` transmite un `CounterSyncRequest`, el firmware evalúa los valores entrantes mediante `counters_validate_sync_acceptance`: <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->

```c
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len)
```

1. **Rechazo de Generación Obsoleta**: Si `host_gen < current_gen`, la sincronización se rechaza (`"stale generation"`). Una base de datos obsoleta del PC nunca puede degradar la generación activa del pad. <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
2. **Rechazo de Retroceso No Monotónico**: Si `host_gen == current_gen` y `host_k1 < current_k1` o `host_k2 < current_k2`, la sincronización se rechaza (`"non-monotonic"`). Los contadores no pueden retroceder dentro de la misma generación. <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
3. **Incrementos Generacionales**: Si `host_gen > current_gen` (por ejemplo, tras un restablecimiento intencionado o la importación de una copia de seguridad), la generación más alta reemplaza incondicionalmente a la anterior.

## Atomic Delta Application

Para evitar perder pulsaciones efectuadas por el jugador mientras un paquete de sincronización viaja por el cable USB: <!-- src: /firmware/main/counters/counters.c -->
- El pad no sobrescribe su contador en RAM con el valor del anfitrión.
- En su lugar, aplica la diferencia:
  $$\Delta = \text{host\_target} - \text{snapshot\_at\_read}$$
  $$\text{RAM\_counter} \mathrel{+}= \Delta$$
- Cualquier pulsación registrada por la ISR de hardware entre la recepción del paquete y la adquisición del bloqueo permanece intacta.

## Lazy Quiescence Checkpointing

Las escrituras en flash se minimizan para proteger la vida útil de la flash SPI: <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_checkpoint_due -->
- Los puntos de control están estrictamente bloqueados mientras el dispositivo está en `OPAD_STATE_PLAYING` o `OPAD_STATE_COOLDOWN`.
- Durante el reposo, `counters_checkpoint_due` comprueba si las teclas han permanecido sin tocar durante un periodo de inactividad antes de guardar los contadores modificados en NVS (`NVS_NS_COUNTERS`).
