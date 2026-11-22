---
title: Modelo de contadores y propiedad
description: Cómo registra OPad contadores monotónicos de pulsaciones de 64 bits, vincula teclados a instalaciones de escritorio, resuelve conflictos de propiedad y preserva pulsaciones durante la sincronización.
---

OPad registra conteos de pulsaciones de por vida tanto en el teclado (en flash NVS) como en el PC anfitrión (en SQLite `opad.db`). Para mantener los conteos precisos entre desconexiones, varios PCs y reemplazos de hardware, OPad combina contadores monotónicos de 64 bits, números de generación y un modelo explícito de propiedad por instalación.

## Contadores monotónicos de pulsaciones

Los conteos de por vida se mantienen de forma independiente para Key 1, Key 2 y (en módulos V3 de 3 teclas) Key 3 en [/firmware/main/counters/counters.c](/firmware/main/counters/counters.c):

- **Contadores de 64 bits**: `lifetime_key1`, `lifetime_key2` y `lifetime_key3` son `uint64_t`. Dentro de la misma `counter_generation`, los conteos solo avanzan ([/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c)).
- **Incremento en RAM sin bloqueos**: Las pulsaciones aceptadas en Core 0 incrementan inmediatamente los contadores en RAM sin tocar la flash.
- **Guardado diferido en NVS**: Para proteger la latencia y limitar el desgaste de la flash, `counters_checkpoint_due` en [/firmware/main/counters/counter_sync_rules.c](/firmware/main/counters/counter_sync_rules.c) bloquea escrituras NVS durante `PLAYING` y `COOLDOWN`, y solo guarda en `IDLE` tras un periodo de reposo (o al suspender USB / sincronización explícita / antes de reiniciar).

## Propiedad del teclado e identidad de instalación

Dado que un teclado puede moverse entre ordenadores (PC de casa, portátil, evento LAN), OPad vincula los teclados a instalaciones de escritorio en lugar de sobrescribir conteos silenciosamente ([/desktop/daemon/src/identity.rs](/desktop/daemon/src/identity.rs)):

- **ID de instalación (`install.id`)**: Se genera como un UUIDv4 en el primer arranque del daemon y se almacena en `app_state` de SQLite. Identifica la instalación de escritorio, no el equipo: desinstalar OPad elimina `opad.db` e `install.id`, de modo que reinstalar crea una identidad nueva.
- **`owner_id` en el teclado**: El UUID binario de 16 bytes (`OWNER_ID_LEN = 16`) se escribe en el teclado mediante `ClaimOwnership` y se persiste en `device_config_data_t` en NVS.

### Resolución de propiedad al conectar

Cuando un teclado se conecta, informa su `owner_id` de 16 bytes en `HelloAck` ([/desktop/crates/opad-device/src/lib.rs](/desktop/crates/opad-device/src/lib.rs), [/desktop/daemon/src/ipc_handlers.rs](/desktop/daemon/src/ipc_handlers.rs)):

1. **Teclado sin propietario** (`owner_id` de ceros o vacío): El daemon reclama automáticamente el teclado y guarda su `install.id` de 16 bytes en la NVS del teclado (en `IDLE`).
2. **Propietario coincidente** (`owner_id == esta instalación`): La reconciliación bidireccional normal de contadores continúa automáticamente.
3. **Propietario ajeno** (`owner_id != esta instalación`):
   - **El teclado va por delante o igual en ambas teclas**: `take_over_if_pad_is_ahead` en [/desktop/daemon/src/ipc_handlers.rs](/desktop/daemon/src/ipc_handlers.rs) reclama automáticamente el teclado conservando los contadores del teclado, por lo que mover tu teclado a un PC nuevo o reinstalar conserva cada pulsación sin avisos.
   - **El PC va por delante en al menos una tecla**: El daemon activa `pending_takeover` (`TakeoverPrompt`) y `foreign_pad = true`, pausa la sincronización de contadores y pregunta al usuario en `opad-gui` (vía `ResolveTakeover`):
     - Asumir la propiedad conservando los contadores del teclado (`take_over: true, keep_device_counters: true`).
     - Asumir la propiedad enviando los contadores de este PC (`take_over: true, keep_device_counters: false`).
     - Dejar el teclado sin tocar (`take_over: false`): el teclado sigue siendo un teclado USB de 1000 Hz no gestionado mientras `foreign_pad` permanece activo.

## Secuencias de generación y reconciliación

La reconciliación de contadores en [/desktop/crates/opad-storage/src/lib.rs](/desktop/crates/opad-storage/src/lib.rs) (`reconcile_counters`) usa `counter_generation` para distinguir el juego normal de reinicios o importaciones de copias intencionadas:

```rust
if pc.counter_generation > esp.counter_generation {
    pc.clone()
} else if esp.counter_generation > pc.counter_generation {
    esp.clone()
} else {
    // Same generation: component-wise max(pc, esp) for Key 1 and Key 2
}
```

1. **Gana la generación mayor**: Reiniciar contadores (`opadctl reset --yes` o **Device** -> **Reset lifetime counters**) o importar una copia de seguridad incrementa `counter_generation`, permitiendo que conteos menores o cero sustituyan legítimamente a la generación anterior.
2. **Misma generación (máximo por componente)**: `lifetime_key1 = max(pc.k1, esp.k1)` y `lifetime_key2 = max(pc.k2, esp.k2)`, fusionando automáticamente las pulsaciones sin conexión hechas mientras estaba desconectado del PC.
3. **Preservación de incrementos en el teclado**: En [/firmware/main/counters/counters.c](/firmware/main/counters/counters.c) (`counters_sync_from_host`), aplicar una sincronización del anfitrión no sobrescribe a ciegas los contadores en RAM con un valor absoluto. `rebase_lifetime_presses` (y `key_event_add_lifetime_key3(*k3 - current.lifetime_key3)`) aplica la diferencia relativa a la instantánea tomada antes de validar para preservar cualquier tecla pulsada mientras la sincronización estaba en vuelo.
