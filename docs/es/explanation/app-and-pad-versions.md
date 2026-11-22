---
title: Interoperabilidad de Versiones de la Aplicación y del Pad
description: Negociación de protocolo, reglas de compatibilidad hacia atrás y hacia adelante, autodetección de enmarcado y gestión de desajuste de versiones entre el software de escritorio y el firmware del pad.
category: explanation
source_files:
  - /desktop/crates/opad-protocol/src/lib.rs
  - /firmware/main/protocol/protocol.c
  - /desktop/daemon/src/sync.rs
  - /desktop/crates/opad-device/src/lib.rs
---

Una piedra angular de la arquitectura de OPad son las **actualizaciones independientes**: la aplicación de escritorio y el firmware del pad se actualizan de forma independiente (Regla de Producto 3). Nunca se le debe decir a un usuario que "actualice ambos a la vez". Cada revisión del protocolo, del formato wire o del enmarcado está diseñada para funcionar bidireccionalmente: una suite de escritorio completamente nueva funciona sin problemas con un firmware más antiguo, y un firmware actualizado sigue siendo totalmente funcional con compilaciones anteriores de escritorio. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->

## Compatibility Dimensions

La interoperabilidad abarca tres capas bien diferenciadas:
1. **Capa de Teclado USB HID Físico**: Completamente independiente del software. El pad enumera como un teclado estándar USB HID a 1000 Hz sin necesidad de daemon, controladores ni procesos en segundo plano en Linux o Windows (Regla de Producto 2).
2. **Capa de Transporte Serie y Enmarcado**: Autonegociación dinámica entre codificaciones de trama legacy (sin marcar), marked (`0xAA, 0x55`) y checked (`0xAA, 0x5A` + CRC-16). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC --> <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->
3. **Capa de Aplicación y Protobuf**: Identificación de versión, indicadores de negociación de capacidades (`frame_crc`) y tolerancia a campos desconocidos en protobuf. <!-- src: /protocol/opad.proto#Hello -->

## Protocol Version Negotiation

La comunicación comienza con el intercambio inicial `Hello` / `HelloAck`: <!-- src: /firmware/main/protocol/protocol.c#protocol_send_hello_ack -->

- **Handshake del Escritorio**:
  Al detectar el puerto serie CDC del pad, `opad-daemon` transmite un mensaje `Hello` envuelto en enmarcado Marked (`0xAA, 0x55`). En este paquete, el escritorio declara su versión (`client_version`) y solicita enmarcado verificado (`frame_crc = true`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
- **Respuesta del Firmware**:
  El firmware analiza la trama entrante. Inspecciona el tipo de trama del anfitrión y responde consecuentemente:
  - Si el anfitrión envió una trama legacy, el pad responde con una trama legacy.
  - Si el anfitrión envió una trama marcada con `frame_crc = true`, el pad responde con `HelloAck.frame_crc = true` en una trama marcada, y luego pasa sus motores de TX y RX a enmarcado checked (`0xAA, 0x5A`) a partir del siguiente mensaje. <!-- src: /firmware/main/protocol/protocol.c#s_crc_answer -->
  - Si el anfitrión envía `frame_crc = false` (clientes de escritorio más antiguos), el pad permanece en enmarcado marked (`0xAA, 0x55`) indefinidamente.

## Protobuf Forward & Backward Compatibility

El protocolo está definido con Google Protocol Buffers (proto3), que proporciona reglas integradas de evolución sobre el cable: <!-- src: /protocol/opad.proto -->

- **Escritorio Nuevo con Firmware Antiguo**:
  Cuando un escritorio moderno envía mensajes que contienen nuevos campos de configuración (como modos de Rapid Trigger para Hall Effect o acciones de deslizamiento personalizadas), el firmware anterior simplemente omite las etiquetas protobuf no reconocidas durante la decodificación nanopb sin fallar ni cerrarse. El pad aplica los campos que comprende y acusa recibo de la solicitud.
- **Escritorio Antiguo con Firmware Nuevo**:
  Cuando un software de escritorio antiguo se conecta a un firmware más nuevo, el pad genera respuestas `DeviceToHost` que contienen campos opcionales más recientes. El escritorio antiguo ignora las etiquetas no reconocidas y extrae de forma segura los datos estándar de estado y contadores.
- **Asignaciones Estáticas**:
  Para proteger los límites de memoria del firmware embebido, se compilan límites superiores fijos en nanopb a través de `protocol/opad.options` (por ejemplo, longitudes máximas de cadenas, matrices repetidas de tamaño fijo). Las cargas que excedan estos límites se rechazan limpiamente antes del análisis sintáctico. <!-- src: /protocol/opad.options -->

## Incompatible Devices & Version Mismatch Handling

La versión del protocolo wire se rastrea mediante `DEVICE_PROTOCOL_VERSION` (actualmente `1`): <!-- src: /desktop/crates/opad-protocol/src/lib.rs#DEVICE_PROTOCOL_VERSION -->

- Si un pad conectado devuelve un `HelloAck` donde `protocol_version` no coincide con la versión mayor admitida por el daemon, este marca el pad como `IncompatibleDevice` (`desktop/crates/opad-model/src/lib.rs`): <!-- src: /desktop/crates/opad-model/src/lib.rs#IncompatibleDevice -->
  ```rust
  pub struct IncompatibleDevice {
      pub firmware_version: String,
      pub protocol_version: u32,
  }
  ```
- **Fallback Garantizado**:
  Un pad incompatible **nunca se bloquea ni queda inservible**. El daemon no intenta sincronizar contadores, sobrescribir configuraciones ni enviarle telemetría. Sin embargo, el pad sigue funcionando normalmente como un teclado USB HID a 1000 Hz. La interfaz gráfica muestra un aviso informativo aconsejando al usuario sobre la divergencia de versiones sin interrumpir las partidas activas.
