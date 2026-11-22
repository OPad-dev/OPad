---
title: Interoperabilidad de versiones de la app y del teclado
description: Negociación de protocolo, reglas de compatibilidad hacia atrás y hacia adelante, autodetección de entramado y manejo de discrepancias de versión entre el software de escritorio y el firmware del teclado.
sidebar:
  order: 4
---

La suite de escritorio de OPad y el firmware del teclado se actualizan de forma independiente (AGENTS.md §1.3). Nunca se exige a los usuarios actualizar ambos a la vez. El entramado, las banderas de capacidad y los campos protobuf están diseñados para interoperar en ambos sentidos: una aplicación de escritorio nueva funciona con firmware antiguo, y un firmware nuevo funciona con versiones antiguas de la aplicación. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->

## Dimensiones de compatibilidad

La interoperabilidad opera en tres capas:

1. **Capa física de teclado HID**: Independiente del software anfitrión. El teclado se enumera como un teclado USB HID estándar a 1000 Hz sin requerir daemon, GUI ni controlador en Linux o Windows (AGENTS.md §1.2). <!-- src: /firmware/main/usb/usb_descriptors.c -->
2. **Capa de transporte serie y entramado**: Negociación dinámica entre los formatos de trama legacy (sin marcar), marked (`0xAA, 0x55`) y checked (`0xAA, 0x5A` + CRC-16) ([`Framing`](/desktop/crates/opad-protocol/src/lib.rs), [`frame_parser_feed()`](/firmware/main/protocol/frame_parser.c)).
3. **Capa de aplicación y Protobuf**: Comprobación de versión mayor del protocolo (`DEVICE_PROTOCOL_VERSION`), banderas de capacidad en el apretón de manos [`Hello`](/protocol/opad.proto) y [`HelloAck`](/protocol/opad.proto), y omisión de campos desconocidos de proto3.

## Negociación de versión de protocolo y entramado

Cada sesión USB CDC comienza con un apretón de manos `Hello` / `HelloAck` ([`Handshake`](/desktop/crates/opad-device/src/lib.rs), [`protocol_send_hello_ack()`](/firmware/main/protocol/protocol.c)):

- **Sondeo del anfitrión (`Hello`)**:
  Mientras se desconoce el entramado del teclado, el anfitrión envía un `Hello` cada 400 ms (`HELLO_RETRY`), alternando entre entramado marked (`0xAA, 0x55`) en intentos pares y legacy en intentos impares ([`hello_framing()`](/desktop/crates/opad-device/src/lib.rs)). La carga de `Hello` establece `protocol_version = 1`, `client_version`, `he_calibration = true` y `frame_crc = true` ([`hello_message()`](/desktop/crates/opad-device/src/lib.rs)).
- **Fijación de entramado y respuesta del teclado (`HelloAck`)**:
  Hasta que se decodifica la primera trama válida, el firmware no envía nada (`s_host_framing_known = false`) para que las tramas no solicitadas no desincronicen a un anfitrión antiguo ([`protocol.c`](/firmware/main/protocol/protocol.c)). En el primer mensaje decodificado, el firmware fija `s_host_framing` y bloquea su analizador a esa familia:
  - Si el anfitrión envió una trama **legacy**, el teclado responde en legacy e ignora cabeceras marked/checked durante el resto de la conexión.
  - Si el anfitrión envió una trama **marked** con `Hello.frame_crc = true`, el teclado responde con `HelloAck` (`frame_crc = true`) en una trama **marked** y después activa el entramado checked (`s_tx_crc = true`, `0xAA, 0x5A` + CRC-16) para todas las tramas siguientes. Los analizadores marked de ambos lados aceptan tanto tramas marked como checked.
  - Si el anfitrión envió `Hello.frame_crc = false` (o el teclado ejecuta firmware antiguo que ignora `Hello.frame_crc` y responde `frame_crc = false`), ambos lados permanecen en entramado marked (`0xAA, 0x55`).
- **Negociación de capacidades**:
  [`HelloAck`](/protocol/opad.proto) anuncia funciones opcionales del firmware (como `running_partition`, `key_press_times`, `board_test`, `builtin_animations`, `restart`, `he_input`, `he_calibration`, `he_live`, `tap_rate_per_second`, `key_count`, `input_module` y `frame_crc`). El firmware antiguo omite campos nuevos, que se decodifican como `false`/`0`/vacío en el anfitrión para que la aplicación degrade con elegancia (por ejemplo, recurriendo a `tosu` para tiempos de ritmo de pulsación cuando `key_press_times` es falso).

## Compatibilidad hacia adelante y hacia atrás en Protobuf

El esquema del cable usa Protocol Buffers v3 ([`protocol/opad.proto`](/protocol/opad.proto)):

- **Escritorio nuevo con firmware antiguo**:
  Cuando una aplicación nueva envía campos que el firmware antiguo no reconoce (por ejemplo ajustes Hall Effect o modificadores de deslizamiento), nanopb omite las etiquetas desconocidas durante `pb_decode` y aplica los campos que entiende.
- **Escritorio antiguo con firmware nuevo**:
  Cuando una aplicación antigua se conecta a un firmware nuevo, prost ignora los campos desconocidos en los mensajes `DeviceToHost` y extrae los que conoce.
- **Límites estáticos en el firmware**:
  [`protocol/opad.options`](/protocol/opad.options) establece límites fijos `max_size` y `max_count` para nanopb de modo que el firmware decodifica en búferes estáticos sin asignación en el heap. Las tramas más largas que `FRAME_MAX_PAYLOAD` (8186 bytes) son descartadas por [`frame_parser_feed()`](/firmware/main/protocol/frame_parser.c); las cargas que desbordan un límite de campo fallan limpiamente dentro de [`protocol_decode_host_message()`](/firmware/main/protocol/protocol.c) y registran `DIAG_EVENT_DECODE_FAILED`.

## Dispositivos incompatibles y gestión de discrepancias de versión

La versión mayor del protocolo de cable es [`DEVICE_PROTOCOL_VERSION`](/desktop/crates/opad-protocol/src/lib.rs) (`1`):

- Si `HelloAck.protocol_version` no es igual a `DEVICE_PROTOCOL_VERSION`, el daemon registra [`IncompatibleDevice`](/desktop/crates/opad-model/src/lib.rs) en [`RuntimeController::on_event()`](/desktop/daemon/src/runtime.rs), restaura su estado de contadores previo al apretón de manos y deja `device_connected = false`:
  ```rust
  pub struct IncompatibleDevice {
      pub firmware_version: String,
      pub protocol_version: u32,
  }
  ```
- **Respaldo seguro**:
  Un teclado incompatible sigue funcionando como teclado USB HID a 1000 Hz. El daemon no sincroniza contadores, no sobrescribe su configuración ni le envía telemetría. `opadctl status` imprime una línea de advertencia `Incompatible:` ([`desktop/cli/src/main.rs`](/desktop/cli/src/main.rs)), y `opad-gui` muestra un aviso ([`desktop/gui/src/main.rs`](/desktop/gui/src/main.rs)). Ambos indican el protocolo del teclado y el requerido, `DEVICE_PROTOCOL_VERSION`; la versión del IPC entre GUI y daemon (`IPC_PROTOCOL_VERSION`) no interviene. <!-- src: /desktop/gui/src/main.rs#incompatible_banner, /desktop/cli/src/main.rs#Commands, /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->
