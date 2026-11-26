---
title: Subsistema de contadores del firmware
description: Referencia de implementación de los contadores de pulsaciones monótonos, las reglas de validación de la sincronización, el rebase atómico por diferencias y los checkpoints perezosos en NVS.
sidebar:
  order: 7
---

El subsistema de contadores guarda en NVS (`NVS_NS_COUNTERS` / `"opad"`) los recuentos de pulsaciones de por vida de 64 bits de la Key 1, la Key 2 y la Key 3, mientras mantiene en RAM, sin bloqueos y en el núcleo 0, todos los incrementos en vivo ([/firmware/main/counters/counters.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counters.c), [/firmware/main/counters/counter_sync_rules.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counter_sync_rules.c)).

## Almacenamiento y estado en RAM (`counters.c`)

- Los totales de pulsaciones en vivo están en contadores atómicos `uint64_t` dentro de [/firmware/main/input/key_event.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/key_event.c) (`s_lifetime_presses[KEY_ID_COUNT]`).
- `counters.c` lleva `s_generation`, `s_last_saved_key1`, `s_last_saved_key2`, `s_last_saved_key3` y `s_nvs_writes`, que se guardan en las claves NVS `"generation"`, `"key1_cnt"`, `"key2_cnt"` y `"key3_cnt"` (esta última solo se escribe si la Key 3 no es cero o ya se había guardado antes).
- Un mutex/semáforo de FreeRTOS, `s_lock`, serializa los checkpoints en NVS y las sincronizaciones con el host en el núcleo 1; las ISR y las tareas de teclas del núcleo 0 nunca toman `s_lock`.

## Validación de la sincronización (`counter_sync_rules.c`)

Los valores de un `CounterSyncRequest` entrante se comprueban con `counters_validate_sync_acceptance` en [/firmware/main/counters/counter_sync_rules.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counter_sync_rules.c):

```c
bool counters_validate_sync_acceptance(
    uint32_t current_gen, uint64_t current_k1, uint64_t current_k2,
    uint32_t host_gen, uint64_t host_k1, uint64_t host_k2,
    bool force, char *err_msg, size_t err_msg_len);
```

Salvo que `force` sea verdadero:
1. `host_gen < current_gen` -> rechazada (`"stale generation"`).
2. `host_gen == current_gen` y (`host_k1 < current_k1` o `host_k2 < current_k2`) -> rechazada (`"non-monotonic"`).
3. `host_gen > current_gen` -> aceptada sin condiciones (permite los reinicios autorizados y las restauraciones de copias de seguridad).

`counters_sync_from_host` también rechaza las sincronizaciones cuando `runtime_get_state() != OPAD_STATE_IDLE` (`"not idle"`) o cuando la NVS no se pudo inicializar (`"nvs unavailable"`).

## Rebase atómico por diferencias

Para conservar los toques hechos mientras un mensaje de sincronización está en camino, `counters_sync_from_host` en [/firmware/main/counters/counters.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counters.c) toma una instantánea antes de validar y aplica solo la diferencia mediante `rebase_lifetime_presses` (y `key_event_add_lifetime_key3(*k3 - current.lifetime_key3)` módulo `2^64`):

```text
delta = target - snapshot
RAM_counter += delta
```

Cualquier pulsación aceptada entre `counters_get(&current)` y `rebase_lifetime_presses` sigue contada.

## Checkpoints perezosos en reposo

`checkpoint_locked` rechaza las escrituras siempre que `runtime_get_state() != OPAD_STATE_IDLE` y no escribe si la RAM coincide con `s_last_saved_key*` (salvo con `force`). En `IDLE`, `runtime_supervisor_task` guarda un checkpoint cuando:

- `counters_checkpoint_due()` ([/firmware/main/counters/counter_sync_rules.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/counters/counter_sync_rules.c)) lo indica: ninguna pulsación durante `COUNTERS_CHECKPOINT_QUIET_US` (30 s), o el último checkpoint tiene más de `COUNTERS_CHECKPOINT_MAX_AGE_US` (300 s);
- lo ha pedido una suspensión USB (`counters_request_checkpoint()` desde `usb_event_handler` en [/firmware/main/app_main.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/app_main.c));
- está a punto de ejecutarse un reinicio pedido por el host.

Una sincronización correcta con el host y `counters_reset()` (que incrementa la generación y lleva todas las teclas a 0) escriben al momento con `checkpoint_locked(true)`. Sin NVS disponible (`CONFIG_OPAD_TEST_FAIL_NVS` lo demuestra), los contadores siguen funcionando solo en RAM.

Cómo decide el escritorio qué recuentos ganan está en [Contadores y propiedad](../../explanation/counters-and-ownership.md).
