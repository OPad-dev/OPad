---
title: Referencia del Protocolo Serie
description: Especificación del enmarcado serie CDC, negociación y formato wire de Protocol Buffers entre anfitrión y pad.
---

La suite de escritorio de OPad se comunica con el pad a través de serie USB CDC-ACM a 115200 baudios (8N1). Todos los mensajes bidireccionales de control se codifican utilizando Google Protocol Buffers (proto3) encapsulados en tramas delimitadas por longitud. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->

## Framing Layer

El protocolo define tres variantes distintas de enmarcado para mantener la compatibilidad hacia atrás y hacia adelante entre el firmware y los clientes de escritorio. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->

### Framing Variants

| Variante | Bytes Mágicos | Campo de Longitud | Campo de Carga Útil | Campo de Checksum | Cabecera Total |
|---|---|---|---|---|---|
| **Legacy** | Ninguno | `uint32` (little-endian) | Carga protobuf en bruto | Ninguno | 4 bytes |
| **Marked** | `0xAA, 0x55` | `uint16` (little-endian) | Carga protobuf en bruto | Ninguno | 4 bytes |
| **Checked** | `0xAA, 0x5A` | `uint16` (little-endian) | Carga protobuf en bruto | `uint16` (CRC-16 LE) | 4 bytes (+ 2 cola) |

<!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
<!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->
<!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

- **Enmarcado Legacy**: Utilizado por las compilaciones iniciales de firmware. La longitud es un entero de 32 bits sin signo little-endian seguido inmediatamente por los bytes de protobuf. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->
- **Enmarcado Marked**: Comienza con bytes mágicos de sincronización `[0xAA, 0x55]` (`FRAME_MAGIC`) seguidos de una longitud de 16 bits en little-endian. Esto protege contra flujos de bytes espurios al conectarse a un puerto serie activo. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
- **Enmarcado Checked**: Comienza con bytes mágicos `[0xAA, 0x5A]` (`FRAME_MAGIC_CHECKED`) seguidos de una longitud de 16 bits en little-endian, la carga protobuf y una cola CRC-16 de 2 bytes. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->

### CRC-16 Specification

Las tramas Checked emplean el estándar CRC-16/CCITT-FALSE: <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16 -->
- **Polinomio**: `0x1021` ($x^{16} + x^{12} + x^5 + 1$)
- **Valor Inicial**: `0xFFFF`
- **RefIn**: `false` (sin reflexión de bits)
- **RefOut**: `false`
- **XorOut**: `0x0000`
- **Cobertura**: Calculada sobre el campo de longitud de carga de 2 bytes concatenado con los bytes de la carga útil (los bytes mágicos de cabecera están excluidos). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16 --> <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->
- **Codificación**: Añadido al final de la trama en orden little-endian. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#encode_host_message -->

### Frame Size Limits

- **Sobrecarga de cabecera**: 4 bytes (`HEADER_BYTES`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#HEADER_BYTES -->
- **Sobrecarga de cola CRC**: 2 bytes (`CRC_BYTES`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#CRC_BYTES -->
- **Tamaño máximo de carga útil**: 8186 bytes (`MAX_PAYLOAD_BYTES`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#MAX_PAYLOAD_BYTES -->

Las tramas que excedan los 8186 bytes de carga útil o fallen la validación CRC se descartan, reiniciando el analizador para buscar la siguiente marca mágica de sincronización. <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

## Framing Negotiation

Para mantener una interoperabilidad estricta (Regla de Producto 3), la comunicación comienza en enmarcado marked y negocia el enmarcado checked dinámicamente: <!-- src: /firmware/main/protocol/protocol.c#s_crc_answer -->

1. El anfitrión se conecta y envía una trama `HostToDevice` que contiene un mensaje `Hello` envuelto en una **trama Marked** (`0xAA, 0x55`). En el mensaje, `Hello.frame_crc` se establece en `true`. <!-- src: /protocol/opad.proto#Hello -->
2. El pad analiza la trama marcada. Si el firmware admite enmarcado checked, establece `HelloAck.frame_crc = true` y responde con un `DeviceToHost` que contiene `HelloAck` encapsulado en una **trama Marked** (`0xAA, 0x55`). <!-- src: /firmware/main/protocol/protocol.c#protocol_send_hello_ack -->
3. Tras transmitir la trama marcada `HelloAck`, tanto el anfitrión como el pad cambian sus estados de transmisión y recepción a **enmarcado Checked** (`0xAA, 0x5A`). <!-- src: /firmware/main/protocol/protocol.c#s_tx_crc -->
4. Si un pad más antiguo no admite CRC o un anfitrión anterior envía `frame_crc = false`, el enmarcado marked permanece activo durante toda la sesión. Si llegan tramas legacy, el pad responde utilizando el enmarcado legacy. <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

## Top-Level Envelopes

Toda la comunicación del protocolo utiliza dos envoltorios raíz de protobuf que contienen números de seguimiento secuenciales y campos de carga útil `oneof`. <!-- src: /protocol/opad.proto#HostToDevice --> <!-- src: /protocol/opad.proto#DeviceToHost -->

```protobuf
message HostToDevice {
    uint32 sequence_number = 1;
    oneof payload {
        Hello hello = 2;
        SetConfig set_config = 3;
        TimeSync time_sync = 4;
        GameplayDisplayState gameplay_state = 5;
        CounterSyncRequest counter_sync = 6;
        bool request_status = 7;
        HostStatus host_status = 8;
        bool reset_latency_stats = 9;
        DataUpdate data_update = 10;
        SetLayout set_layout = 11;
        uint32 reset_layout = 12;
        bool request_logs = 13;
        ClaimOwnership claim_ownership = 14;
        DetectPinRequest detect_pin = 15;
        bool run_board_test = 16;
        PlayAnimation play_animation = 17;
        bool restart = 18;
        bool start_he_calibration = 19;
        bool cancel_he_calibration = 20;
        bool he_live_view = 21;
    }
}

message DeviceToHost {
    uint32 sequence_number = 1;
    oneof payload {
        HelloAck hello_ack = 2;
        DeviceStatus status = 3;
        ConfigAck config_ack = 4;
        CounterSyncResponse counter_sync_resp = 5;
        LogEventBatch log_batch = 6;
        LayoutAck layout_ack = 7;
        DetectPinResponse detect_pin_resp = 8;
        KeyPressBatch key_presses = 9;
        BoardTestResult board_test = 10;
        HeCalibrationProgress he_calibration_progress = 11;
        HeLiveSample he_live_sample = 12;
    }
}
```

## Protocol Message Catalog

### Session & Identity Messages

- **`Hello` / `HelloAck`**: Establece la versión del protocolo (`ProtocolVersion`), la cadena de versión del cliente, la versión de firmware del pad, el identificador de serie del dispositivo, la ranura de partición en ejecución (`ota_0` / `ota_1`) y el identificador de propietario. <!-- src: /protocol/opad.proto#Hello --> <!-- src: /protocol/opad.proto#HelloAck -->
- **`ClaimOwnership`**: Empareja una identidad de anfitrión (`owner_id`) con el pad. El pad persiste los metadatos de propietario en NVS. <!-- src: /protocol/opad.proto#ClaimOwnership -->
- **`DeviceStatus`**: Enviado en respuesta a `request_status = true`. Informa del estado del dispositivo (`DeviceState`), el tipo de módulo de entrada activo (`InputModule`), conteos de pulsaciones, tiempo encendido y estado de Hall Effect. <!-- src: /protocol/opad.proto#DeviceStatus -->
- **`HostStatus`**: Enviado periódicamente por el daemon para mantener al pad informado sobre el perfil activo del anfitrión, el volumen de audio y el estado de reproducción. <!-- src: /protocol/opad.proto#HostStatus -->
- **`restart`**: Indica al pad que realice un reinicio limpio una vez en reposo (nunca a mitad de un mapa). <!-- src: /protocol/opad.proto#HostToDevice -->

### Configuration Messages

- **`SetConfig` / `ConfigAck`**: Transmite una carga útil `ConfigPayload` actualizada que contiene algoritmos de rebote, ajustes de LED, diseños de pantalla y parámetros de Hall Effect (`HeKeySettings`). En conformidad con los requisitos de latencia, la escritura en NVS se pospone si el dispositivo está jugando activamente un mapa (`PLAYING` / `COOLDOWN`). <!-- src: /protocol/opad.proto#SetConfig --> <!-- src: /protocol/opad.proto#ConfigAck --> <!-- src: /firmware/main/protocol/protocol.c#apply_he_settings -->
- **`ConfigPayload`**: Contiene la estructura completa de ajustes incluyendo `key1_gpio`, `key2_gpio`, `debounce_ms`, `led_brightness`, `screen_timeout_s` y definiciones de teclas de juego. <!-- src: /protocol/opad.proto#ConfigPayload -->

### Gameplay & Screen Messages

- **`TimeSync`**: Sincroniza la marca de tiempo del reloj del pad con el tiempo monotónico del sistema anfitrión. <!-- src: /protocol/opad.proto#TimeSync -->
- **`GameplayDisplayState`**: Transmite metadatos del mapa activo actualmente (título de canción, artista, nombre de dificultad, duración del mapa) para actualizar la pantalla LCD. <!-- src: /protocol/opad.proto#GameplayDisplayState -->
- **`DataUpdate`**: Transmisión de alta frecuencia de estadísticas de juego (UR, combo, precisión, PPM, grado) compuesta por matrices `DataValue`. <!-- src: /protocol/opad.proto#DataUpdate -->
- **`SetLayout` / `LayoutAck`**: Define dinámicamente diseños de pantalla personalizados compuestos por hasta 32 elementos `UiWidget`. <!-- src: /protocol/opad.proto#SetLayout --> <!-- src: /protocol/opad.proto#LayoutAck -->
- **`reset_layout`**: Restablece el identificador de pantalla especificado al diseño por defecto integrado. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`PlayAnimation`**: Desencadena una animación de calificación o celebración en la pantalla del pad. <!-- src: /protocol/opad.proto#PlayAnimation -->

### Hall Effect Calibration & Telemetry Messages

- **`HeKeySettings`**: Punto de actuación por tecla, punto de reinicio, zona muerta y modo de Rapid Trigger (`HeRtMode`). <!-- src: /protocol/opad.proto#HeKeySettings --> <!-- src: /protocol/opad.proto#HeRtMode -->
- **`HeCalibrationStatus`**: Estado actual de calibración (`HeCalStage`), valores ADC en reposo y extremos de recorrido inferior. <!-- src: /protocol/opad.proto#HeCalibrationStatus --> <!-- src: /protocol/opad.proto#HeCalStage -->
- **`start_he_calibration` / `cancel_he_calibration`**: Comandos del anfitrión para iniciar o cancelar la secuencia de calibración interactiva. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`HeCalibrationProgress`**: Difunde el paso actual de la calibración guiada y reporta cualquier error de calibración de hardware (`HeCalError`). <!-- src: /protocol/opad.proto#HeCalibrationProgress --> <!-- src: /protocol/opad.proto#HeCalError -->
- **`he_live_view` / `HeLiveSample`**: Transmite en tiempo real la distancia normalizada de recorrido y lecturas magnéticas en bruto (~50 Hz) durante la vista del visualizador de Hall Effect en el escritorio. <!-- src: /protocol/opad.proto#HeLiveSample -->

### Counters, Hardware Diagnostics & Self-Test Messages

- **`CounterSyncRequest` / `CounterSyncResponse`**: Reconciliación bidireccional de totales de pulsaciones persistentes de vida útil (`CounterState`). <!-- src: /protocol/opad.proto#CounterSyncRequest --> <!-- src: /protocol/opad.proto#CounterSyncResponse -->
- **`DetectPinRequest` / `DetectPinResponse`**: Herramienta interactiva de prueba de pines donde el anfitrión solicita al pad monitorizar un pin GPIO para una pulsación física de interruptor. <!-- src: /protocol/opad.proto#DetectPinRequest --> <!-- src: /protocol/opad.proto#DetectPinResponse -->
- **`run_board_test` / `BoardTestResult`**: Ejecuta autotests de hardware del pad (escaneo de bus I2C, comunicación con pantalla, respuesta del controlador táctil y canales ADC de Hall Effect). <!-- src: /protocol/opad.proto#BoardTestResult -->
- **`KeyPress` / `KeyPressBatch`**: Lote de diagnóstico de eventos de pulsación recientes, incluyendo marcas de tiempo de pulsación/liberación y duraciones. <!-- src: /protocol/opad.proto#KeyPressBatch -->
- **`LogEvent` / `LogEventBatch`**: Eventos de registro de diagnóstico drenados desde el búfer circular del pad hacia el daemon del anfitrión. <!-- src: /protocol/opad.proto#LogEventBatch -->

## Nanopb Buffer and Field Limits

Las longitudes de campo y los límites de matrices fijas se especifican en `protocol/opad.options`: <!-- src: /protocol/opad.options -->

| Campo | Tamaño Máx / Límite | Tipo de Asignación |
|---|---|---|
| `Hello.client_version` | 32 caracteres | Cadena fija |
| `HelloAck.firmware_version` | 32 caracteres | Cadena fija |
| `HelloAck.board_profile` | 32 caracteres | Cadena fija |
| `HelloAck.device_id` | 32 caracteres | Cadena fija |
| `HelloAck.owner_id` | 16 caracteres | Cadena fija |
| `HelloAck.running_partition` | 17 caracteres | Cadena fija |
| `ClaimOwnership.owner_id` | 16 caracteres | Cadena fija |
| `ConfigAck.message` | 64 caracteres | Cadena fija |
| `GameplayDisplayState.title` | 64 caracteres | Cadena fija |
| `GameplayDisplayState.artist` | 64 caracteres | Cadena fija |
| `GameplayDisplayState.difficulty` | 32 caracteres | Cadena fija |
| `DataValue.text` | 64 caracteres | Cadena fija |
| `DataUpdate.values` | 32 elementos | Mensaje repetido |
| `UiWidget.label` | 32 caracteres | Cadena fija |
| `UiWidget.suffix` | 12 caracteres | Cadena fija |
| `SetLayout.widgets` | 32 elementos | Mensaje repetido |
| `LayoutAck.message` | 64 caracteres | Cadena fija |
| `PlayAnimation.grade` | 4 caracteres | Cadena fija |
| `CounterState.device_id` | 32 caracteres | Cadena fija |
| `CounterSyncResponse.message` | 64 caracteres | Cadena fija |
| `LogEvent.tag` | 16 caracteres | Cadena fija |
| `LogEvent.message` | 64 caracteres | Cadena fija |
| `LogEventBatch.events` | 8 elementos | Mensaje repetido |
| `KeyPressBatch.presses` | 32 elementos | Mensaje repetido |
| `BoardTestResult.message` | 64 caracteres | Cadena fija |
| `HeAdcTest.message` | 48 caracteres | Cadena fija |
| `HeAdcTest.speeds` | 6 elementos | Mensaje repetido |
| `HeAdcTest.channels` | 3 elementos | Mensaje repetido |
| `HeCalibrationProgress.error_message` | 48 caracteres | Cadena fija |

Estos límites garantizan asignaciones estáticas en el firmware del ESP32-S3, evitando la fragmentación dinámica del heap durante la ejecución en tiempo real. <!-- src: /firmware/main/protocol/protocol.c#protocol_encode_device_message -->
