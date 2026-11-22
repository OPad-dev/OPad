---
title: Subsistema de contadores del firmware
description: Referencia de implementación de los contadores monotónicos de pulsaciones, reglas de validación de sincronización, reajuste atómico por diferencias y guardado diferido en NVS.
sidebar:
  order: 7
---

El subsistema de contadores persiste conteos de pulsaciones de por vida de 64 bits para Key 1, Key 2 y Key 3 en NVS (`NVS_NS_COUNTERS` / `"opad"`) manteniendo todos los incrementos en vivo sin bloqueos en RAM en Core 0 ([/firmware/main/counters/counters.c](/firmware/main/counters/counters.c), [/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c)).

## Almacenamiento y estado en RAM (`counters.c`)

- Los totales de pulsaciones en vivo residen en contadores atómicos `uint64_t` dentro de [/firmware/main/input/key_event.c](/firmware/main/input/key_event.c) (`s_lifetime_presses[KEY_ID_COUNT]`).
- `counters.c` lleva el registro de `s_generation`, `s_last_saved_key1`, `s_last_saved_key2`, `s_last_saved_key3` y `s_nvs_writes`, persistidos bajo las claves NVS `"generation"`, `"key1_cnt"`, `"key2_cnt"` y `"key3_cnt"` (escrita solo cuando Key 3 no es cero o ya se guardó antes).
- Un mutex/semáforo FreeRTOS `s_lock` serializa los guardados en NVS y las sincronizaciones del anfitrión en Core 1; las ISR y tareas de teclas de Core 0 nunca adquieren `s_lock`.

## Validación de sincronización (`counter_sync_rules.c`)

Los valores entrantes de `CounterSyncRequest` son comprobados por `counters_validate_sync_acceptance` en [/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c):

```c
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len);
```

Salvo que `force` sea verdadero:
1. `host_gen < current_gen` -> rechazado (`"stale generation"`).
2. `host_gen == current_gen` y (`host_k1 < current_k1` o `host_k2 < current_k2`) -> rechazado (`"non-monotonic"`).
3. `host_gen > current_gen` -> aceptado incondicionalmente (admite reinicios autorizados y restauraciones de copias de seguridad).

`counters_sync_from_host` también rechaza sincronizaciones cuando `runtime_get_state() != OPAD_STATE_IDLE` (`"not idle"`) o cuando NVS falló al inicializarse (`"nvs unavailable"`).

## Reajuste atómico por diferencias

Para preservar pulsaciones realizadas mientras un mensaje de sincronización está en tránsito, `counters_sync_from_host` en [/firmware/main/counters/counters.c](/firmware/main/counters/counters.c) toma una instantánea antes de validar y aplica únicamente la diferencia mediante `rebase_lifetime_presses` (y `key_event_add_lifetime_key3(*k3 - current.lifetime_key3)` módulo `2^64`):

```text
delta = target - snapshot
RAM_counter += delta
```

Cualquier pulsación aceptada entre `counters_get(&current)` y `rebase_lifetime_presses` sigue quedando contabilizada.

## Guardado diferido tras inactividad

`checkpoint_locked` rechaza escrituras siempre que `runtime_get_state() != OPAD_STATE_IDLE` y omite escribir cuando la RAM coincide con `s_last_saved_key*` (salvo con `force`). `runtime_supervisor_task` llama a `counters_checkpoint_due` ([/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c)) en `IDLE` para guardar solo tras una ventana de inactividad, al suspender USB (`counters_request_checkpoint` desde `usb_event_handler`) o antes de un reinicio limpio.
