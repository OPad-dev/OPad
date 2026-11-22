---
title: Modelo de Contadores y Propiedad
description: Explicación profunda de los contadores de pulsaciones monotónicos de por vida, emparejamiento de propiedad del pad, secuencias de generación y mitigación del desgaste de memoria flash.
category: explanation
source_files:
  - /firmware/main/counters/counters.c
  - /firmware/main/counters/counter_sync_rules.c
  - /desktop/daemon/src/sync.rs
  - /desktop/daemon/src/identity.rs
  - /desktop/crates/opad-storage/src/lib.rs
---

OPad rastrea los conteos de pulsaciones de por vida del jugador a través de interruptores físicos y sesiones de juego. Estos contadores sirven como hitos verificables para los jugadores de juegos de ritmo. Para proteger la integridad de los contadores entre diferentes ordenadores, reinstalaciones y sustituciones de hardware, OPad acopla el seguimiento de contadores monotónicos con un modelo explícito de emparejamiento de propiedad. <!-- src: /desktop/daemon/src/identity.rs -->

## Monotonic Keypress Counters

Los totales de pulsaciones de por vida se rastrean de forma independiente para Key 1, Key 2 y la opcional Key 3. <!-- src: /firmware/main/counters/counters.c -->

- **Enteros Monotónicos de 64 Bits**: Los totales de pulsaciones (`lifetime_key1`, `lifetime_key2`, `lifetime_key3`) son enteros sin signo de 64 bits. Los contadores solo avanzan hacia adelante; nunca pueden retroceder dentro de la misma generación de contadores. <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_validate_sync_acceptance -->
- **Ruta Rápida en RAM**: En las rutinas de servicio de interrupción de teclas del firmware (Core 0), las pulsaciones incrementan variables atómicas en RAM sin contención de bloqueos. Los contadores en RAM avanzan continuamente incluso durante el juego activo. <!-- src: /firmware/main/counters/counters.c -->
- **Puntos de Control en Flash**: Confirmar los contadores en la memoria flash SPI no volátil (NVS) desgasta la memoria flash y detiene la ejecución de la CPU. El firmware mitiga el desgaste de flash utilizando puntos de control perezosos (`counters_checkpoint_due` en `firmware/main/counters/counter_sync_rules.c`): <!-- src: /firmware/main/counters/counter_sync_rules.c#counters_checkpoint_due -->
  - Los puntos de control están estrictamente prohibidos durante el juego activo (`PLAYING` / `COOLDOWN`).
  - Durante el reposo, los puntos de control se producen solo después de que las teclas hayan permanecido sin tocar durante un periodo de inactividad, minimizando los ciclos de escritura.

## Pad Ownership & Identity Model

Un pad se conecta con frecuencia a varios ordenadores (por ejemplo, el PC de casa, el portátil de torneos, una configuración LAN). Para evitar sobrescribir silenciosamente los registros de contadores entre diferentes sistemas, OPad implementa un modelo de emparejamiento: <!-- src: /desktop/daemon/src/identity.rs -->

### Installation Identity (install.id)
- Cada instalación de escritorio genera un UUIDv4 persistente en su ejecución inicial, almacenado en SQLite bajo `install.id`. <!-- src: /desktop/daemon/src/identity.rs#INSTALL_ID_KEY -->
- La identidad es **por instalación, no por máquina**. Si un usuario desinstala el software con la opción "eliminar todo", la identidad y su base de datos SQLite se eliminan. Reinstalar genera una identidad nueva. <!-- src: /desktop/daemon/src/identity.rs -->
- El identificador binario de propietario de 16 bytes se envía al pad mediante `ClaimOwnership`. El pad registra este ID de propietario de 16 bytes en NVS. <!-- src: /desktop/daemon/src/identity.rs#OWNER_ID_LEN -->

### Ownership States & Conflict Resolution
Cuando un pad se conecta a `opad-daemon`, informa de su `owner_id` almacenado en `HelloAck`: <!-- src: /desktop/crates/opad-device/src/lib.rs#Ownership -->
1. **Pad sin Reclamar (`owner_id` está vacío)**: El daemon reclama automáticamente el pad, escribiendo su identidad de instalación de 16 bytes en NVS.
2. **Propietario Coincidente (`owner_id == esta instalación`)**: El pad pertenece a este PC. La sincronización bidireccional normal de contadores continúa automáticamente.
3. **Propietario Distinto (`owner_id != esta instalación`)**: El pad fue emparejado previamente con otro PC.
   - Si los contadores de por vida del pad son mayores o iguales que los registros de la base de datos de este PC en ambas teclas, el pad se adopta automáticamente (`take_over_if_pad_is_ahead`), garantizando que no se pierda ningún progreso al cambiar de ordenador. <!-- src: /desktop/daemon/src/ipc_handlers.rs#take_over_if_pad_is_ahead -->
   - Si este PC tiene conteos más altos que el pad, se marca un conflicto de propiedad, preguntando al usuario (`TakeoverPrompt`) en la interfaz gráfica o en `opadctl status`:
     - **Tomar posesión y adoptar contadores del pad**: Actualiza la base de datos local con los números del pad.
     - **Tomar posesión y restaurar contadores del PC**: Escribe la base de datos de contadores más altos de este PC en el pad.
     - **Dejar sin cambios**: El pad continúa funcionando como un teclado HID a 1000 Hz no gestionado; no se realiza sincronización.

## Generation Sequences & Counter Reconciliation

Para gestionar restablecimientos intencionados de contadores o restauraciones de copias de seguridad sin violar la monotonicidad, los contadores se rigen por un **número de secuencia de generación** (`counter_generation`): <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->

```rust
pub fn reconcile_counters(pc: &CounterState, esp: &CounterState) -> CounterState {
    if pc.counter_generation > esp.counter_generation {
        pc.clone()
    } else if esp.counter_generation > pc.counter_generation {
        esp.clone()
    } else {
        CounterState {
            lifetime_key1: pc.lifetime_key1.max(esp.lifetime_key1),
            lifetime_key2: pc.lifetime_key2.max(esp.lifetime_key2),
            ...
        }
    }
}
```

### Reconciliation Rules
1. **La Generación Más Reciente Gana**: Si un usuario realiza un restablecimiento autorizado (`opadctl reset --confirm`) o restaura una copia de seguridad, `counter_generation` se incrementa. Cualquier estado de contador con un número de generación más alto reemplaza incondicionalmente a las generaciones anteriores, permitiendo que los contadores se reinicien limpiamente a cero. <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->
2. **Generación Idéntica (Regla de Superconjunto)**: Si tanto el PC como el pad comparten la misma generación, el algoritmo de reconciliación calcula el máximo componente a componente:
   $$\text{lifetime\_key1} = \max(\text{pc.k1}, \text{esp.k1})$$
   $$\text{lifetime\_key2} = \max(\text{pc.k2}, \text{esp.k2})$$
   Cualquier pulsación efectuada en el pad mientras estaba desconectado del PC se integra de manera transparente en la base de datos del escritorio.
3. **Preservación de Deltas en el Firmware**: Cuando el pad recibe un comando de sincronización de contadores (`CounterSyncRequest`), no sobrescribe sus contadores en RAM con el número absoluto recibido. En su lugar, calcula el delta y lo aplica:
   $$\text{RAM\_counter} \mathrel{+}= (\text{target\_count} - \text{snapshot\_count})$$
   Cualquier pulsación física efectuada por el jugador mientras la trama de sincronización cruzaba el bus serie USB permanece intacta. <!-- src: /firmware/main/counters/counters.c -->
