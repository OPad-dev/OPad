---
title: Referencia del protocolo serie
description: Especificación del entramado serie CDC, negociación y formato de cable Protocol Buffers entre anfitrión y teclado.
---

La suite de escritorio de OPad se comunica con el teclado mediante serie USB CDC-ACM a 115200 baudios. Los mensajes de control y telemetría se codifican como Protocol Buffers v3 (`proto3`) dentro de tramas delimitadas por longitud. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing, /desktop/crates/opad-device/src/lib.rs -->

## Capa de entramado

Existen tres variantes de entramado para mantener compatibilidad bidireccional entre aplicaciones de escritorio y firmwares antiguos y nuevos. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#Framing -->

### Variantes de entramado

| Variante | Bytes mágicos | Campo de longitud | Carga útil | Campo de comprobación | Cabecera / Cola |
|---|---|---|---|---|---|
| **Legacy** | Ninguno | `uint32` (little-endian, `>= 2`) | Carga protobuf cruda | Ninguno | 4 bytes / 0 bytes |
| **Marked** | `0xAA, 0x55` | `uint16` (little-endian) | Carga protobuf cruda | Ninguno | 4 bytes / 0 bytes |
| **Checked** | `0xAA, 0x5A` | `uint16` (little-endian) | Carga protobuf cruda | `uint16` (CRC-16 LE) | 4 bytes / 2 bytes |

<!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC, /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED, /firmware/main/protocol/frame_parser.c#frame_parser_feed -->

- **Entramado Legacy**: Usado por firmwares y aplicaciones anteriores a las tramas marcadas. Una longitud sin signo de 32 bits little-endian (`2..=8186`, `LEGACY_MIN_PAYLOAD = 2` para que las rachas de ceros no se interpreten como tramas vacías) seguida inmediatamente de la carga protobuf. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#LEGACY_MIN_PAYLOAD -->
- **Entramado Marked**: Empieza con los bytes mágicos `[0xAA, 0x55]` (`FRAME_MAGIC`) seguidos de una longitud little-endian de 16 bits y la carga protobuf. Permite resincronizar rápidamente tras bytes serie sueltos. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC -->
- **Entramado Checked**: Empieza con los bytes mágicos `[0xAA, 0x5A]` (`FRAME_MAGIC_CHECKED`) seguidos de una longitud little-endian de 16 bits, la carga protobuf y una cola CRC-16 little-endian de 2 bytes. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#FRAME_MAGIC_CHECKED -->

### Especificación CRC-16

Las tramas Checked utilizan CRC-16/CCITT-FALSE (valor de comprobación `0x29B1` para `"123456789"`): <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16, /firmware/main/protocol/frame_parser.c#frame_crc16 -->

- **Polinomio**: `0x1021`
- **Valor inicial**: `0xFFFF`
- **RefIn / RefOut**: `false` / `false`
- **XorOut**: `0x0000`
- **Cobertura**: Calculado sobre el campo de longitud de 2 bytes concatenado con los bytes de carga útil (se excluyen los 2 bytes mágicos). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#checked_crc, /firmware/main/protocol/frame_parser.c#checked_crc -->
- **Codificación**: Añadido tras la carga útil en orden de bytes little-endian. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#encode_frame, /firmware/main/protocol/frame_parser.c#frame_write_crc -->

### Límites de tamaño de trama

- **Sobrecarga de cabecera**: 4 bytes (`HEADER_BYTES` / `FRAME_HEADER_SIZE`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#HEADER_BYTES -->
- **Sobrecarga de cola CRC**: 2 bytes (`CRC_BYTES` / `FRAME_CRC_SIZE`) solo en tramas Checked. <!-- src: /desktop/crates/opad-protocol/src/lib.rs#CRC_BYTES -->
- **Tamaño máximo de carga útil**: 8186 bytes (`MAX_PAYLOAD_BYTES = 8192 - 4 - 2` / `FRAME_MAX_PAYLOAD`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#MAX_PAYLOAD_BYTES -->
- **Tiempo límite de trama parcial obsoleta**: 500 ms (`RX_STALE_US = 500000` en firmware, `STALE_PARTIAL_FRAME = 500 ms` en anfitrión). <!-- src: /firmware/main/protocol/protocol.c#RX_STALE_US, /desktop/crates/opad-device/src/lib.rs#STALE_PARTIAL_FRAME -->

Las tramas con longitudes superiores a 8186 bytes o que fallen la validación CRC se rechazan avanzando un byte la posición de escaneo para seguir recuperando cualquier trama real pegada inmediatamente tras una trama truncada. <!-- src: /firmware/main/protocol/frame_parser.c#frame_parser_feed, /desktop/crates/opad-protocol/src/lib.rs#decode_frame -->

## Negociación de entramado

Para preservar la compatibilidad bidireccional (AGENTS.md §1.3), el entramado se negocia en cada conexión: <!-- src: /desktop/crates/opad-device/src/lib.rs#Handshake, /firmware/main/protocol/protocol.c#s_crc_answer -->

1. Mientras se desconoce el entramado del teclado, el anfitrión envía `Hello` (`frame_crc = true`, `he_calibration = true`) cada 400 ms (`HELLO_RETRY`), alternando entre **Marked** (`0xAA, 0x55`) y **Legacy** hasta que el teclado responde. <!-- src: /desktop/crates/opad-device/src/lib.rs#hello_framing, /desktop/crates/opad-device/src/lib.rs#hello_message -->
2. En el primer mensaje decodificado del anfitrión, el teclado fija esa familia de entramado (`s_host_framing`) y responde usando el mismo entramado. <!-- src: /firmware/main/protocol/protocol.c#s_host_framing -->
3. Si el anfitrión envió un `Hello` Marked con `frame_crc = true` y el firmware admite tramas comprobadas, el teclado envía `HelloAck` con `frame_crc = true` como trama **Marked** y luego cambia TX a entramado **Checked** (`0xAA, 0x5A` + CRC-16) para todos los mensajes siguientes (`s_tx_crc = true`). Al recibir `HelloAck.frame_crc = true`, el anfitrión también cambia su TX a entramado Checked. <!-- src: /firmware/main/protocol/protocol.c#s_tx_crc, /desktop/crates/opad-device/src/lib.rs#Handshake -->
4. Si alguno de los extremos no admite CRC (o si se negoció Legacy), ambos permanecen en Marked o Legacy durante la sesión. Bajar DTR (cerrar el puerto) reinicia el estado de entramado y CRC en el teclado. <!-- src: /firmware/main/usb/usb_cdc.c#tud_cdc_line_state_cb, /firmware/main/protocol/protocol.c -->

## Envolventes de nivel superior

Todos los mensajes del protocolo usan dos envolventes protobuf raíz con un `sequence_number` y un `oneof payload`: <!-- src: /protocol/opad.proto#HostToDevice, /protocol/opad.proto#DeviceToHost -->

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

## Catálogo de mensajes del protocolo

### Mensajes de sesión e identidad

- **`Hello` / `HelloAck`**: Apretón de manos que transporta `protocol_version`, `client_version`, `firmware_version`, `board_profile`, `device_id`, generación de contadores y totales de Key 1/2, `owner_id` de 16 bytes, `running_partition` (`ota_0` / `ota_1` / `factory`), `current_config`, `input_module` (`InputModule`), `key_count` y banderas de capacidad (`key_press_times`, `board_test`, `builtin_animations`, `restart`, `he_input`, `he_calibration`, `he_live`, `tap_rate_per_second`, `frame_crc`). <!-- src: /protocol/opad.proto#Hello, /protocol/opad.proto#HelloAck -->
- **`ClaimOwnership`**: Envía un `owner_id` de 16 bytes para emparejar el teclado con una instalación anfitriona en NVS. Solo se atiende en `IDLE` y rechaza IDs vacíos o de ceros. <!-- src: /protocol/opad.proto#ClaimOwnership, /firmware/main/config/device_config.c#device_config_claim_owner -->
- **`DeviceStatus`**: Devuelto en respuesta a `request_status = true`. Informa `uptime_seconds`, `state` (`DeviceState`), `brightness`, `display_asleep`, `lifetime_key1`/`2`/`3`, `map_key1`/`2`/`3`, estadísticas de latencia del dispositivo (`latency_samples`, `p50`, `p99`, `p999`, `max`, `hid_deferred_reports`), `display_ok`, `nvs_ok`, `nvs_writes` y `he_calibration` (`HeCalibrationStatus`). <!-- src: /protocol/opad.proto#DeviceStatus -->
- **`HostStatus`**: Enviado cada 1 segundo y en transiciones de estado para informar al teclado de `tosu_connected`, `playing`, `play_id` (reinicia contadores de pulsaciones de mapa al cambiar) y `osu_active` (si los deslizamientos de volumen actúan sobre osu! o sobre el volumen del sistema). <!-- src: /protocol/opad.proto#HostStatus, /desktop/daemon/src/runtime.rs#HOST_STATUS_INTERVAL -->
- **`restart`**: Solicita un reinicio limpio del firmware (`esp_restart`) una vez que el teclado esté en `IDLE` y su imagen OTA esté confirmada; guarda contadores, configuración, diseños y calibración pendientes antes de reiniciar. <!-- src: /protocol/opad.proto#HostToDevice, /firmware/main/runtime/runtime.c#restart_now -->

### Mensajes de configuración

- **`SetConfig` / `ConfigAck`**: Actualiza ajustes del dispositivo mediante `ConfigPayload` y devuelve estado de éxito, mensaje y `current_config` activa. La persistencia en NVS se difiere hasta `IDLE` si se recibe durante `PLAYING` o `COOLDOWN`. <!-- src: /protocol/opad.proto#SetConfig, /protocol/opad.proto#ConfigAck, /firmware/main/config/device_config.c#write_to_nvs -->
- **`ConfigPayload`**: Transporta `key1_hid_usage`, `key2_hid_usage`, `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`, `press_color_rgb` obsoleto, `key1_gpio`, `key2_gpio`, campos por dirección `SwipeAction` / tecla / modificadores (`swipe_{left,right,up,down}_{action,key,modifiers}`), ajustes Hall Effect por tecla (`key1_he`, `key2_he`, `key3_he`), `key3_hid_usage` opcional y máscara de bits `game_keys` opcional. <!-- src: /protocol/opad.proto#ConfigPayload -->

### Mensajes de juego y pantalla

- **`TimeSync`**: Sincroniza la fecha y hora del teclado (`year`, `month`, `day`, `hour`, `minute`, `second`). <!-- src: /protocol/opad.proto#TimeSync -->
- **`GameplayDisplayState`**: Mensaje heredado de visualización de mapa que transporta `title`, `artist`, `current_pp`, `progress_ratio` (`0.0`–`1.0`), `current_map_presses_k1`/`k2`, `play_id`, `difficulty` y `stars`. <!-- src: /protocol/opad.proto#GameplayDisplayState -->
- **`DataUpdate`**: Transmite valores modificados de fuentes de datos de UI (`repeated DataValue values`, hasta 32 por mensaje), cada uno con un ID `source` (`ui_source_t`) y `oneof value { double number; string text; bool clear; }`. <!-- src: /protocol/opad.proto#DataUpdate, /protocol/opad.proto#DataValue -->
- **`SetLayout` / `LayoutAck`**: Envía un diseño de pantalla personalizado (`screen`, `background` RGB y hasta 32 entradas `UiWidget`) y confirma aceptación o informa errores de validación. <!-- src: /protocol/opad.proto#SetLayout, /protocol/opad.proto#LayoutAck -->
- **`reset_layout`**: Restablece el ID de pantalla indicado (`uint32`) a su diseño predeterminado integrado. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`PlayAnimation`**: Reproduce una animación de resultados (ID `animation`: `0` detiene, `1..127` integradas, `128+n` reservadas) con la cadena de calificación (`"SS"`, `"S"`, `"A"`, ...). Se ignora durante un mapa y se detiene inmediatamente al empezar uno. <!-- src: /protocol/opad.proto#PlayAnimation -->

### Mensajes de calibración y telemetría Hall Effect

- **`HeKeySettings`**: Ajustes por tecla en 0.1% del rango calibrado (`1..1000`): `actuation`, `release_hyst`, `top_reset`, `rt_press`, `rt_release`, `bottom_dead_zone` opcional, `rt_mode` (`HeRtMode`) y `allow_below_noise_floor` opcional. <!-- src: /protocol/opad.proto#HeKeySettings, /protocol/opad.proto#HeRtMode -->
- **`HeCalibrationStatus`**: Incluido en `DeviceStatus`. Informa banderas de calibración y conteos crudos de reposo/fondo por tecla para Keys 1–3, `is_locked`, `is_calibrating`, `noise_floor_rest` y `noise_floor_movement`. <!-- src: /protocol/opad.proto#HeCalibrationStatus -->
- **`start_he_calibration` / `cancel_he_calibration`**: Inicia o cancela la calibración interactiva Hall Effect. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`HeCalibrationProgress`**: Informa la etapa activa (`HeCalStage`), `key_index` (`1`–`3`, `0` en reposo/inactivo/completo), `presses_done` (`0..5`), `error` (`HeCalError`), `error_message`, `raw_value` en vivo y conteos guardados `key{1,2,3}_{rest,full}` al tener éxito. <!-- src: /protocol/opad.proto#HeCalibrationProgress, /protocol/opad.proto#HeCalStage, /protocol/opad.proto#HeCalError -->
- **`he_live_view` / `HeLiveSample`**: Enviar `he_live_view = true` transmite conteos ADC crudos (`key1_raw`, `key2_raw`, `key3_raw`) y estados booleanos pulsados (`key1_down`, `key2_down`, `key3_down`) a ~50 Hz (`HE_LIVE_PERIOD_US = 20000`) durante 2 segundos (`HE_LIVE_KEEPALIVE_US = 2000000`); el anfitrión lo refresca para mantener el flujo y convierte los conteos crudos en recorrido usando `HeCalibrationStatus`. Nunca se transmite durante `PLAYING` ni `COOLDOWN`. <!-- src: /protocol/opad.proto#HeLiveSample, /firmware/main/protocol/protocol.c#HE_LIVE_PERIOD_US -->

### Mensajes de contadores, diagnóstico de hardware y autoprueba

- **`CounterSyncRequest` / `CounterSyncResponse`**: Reconcilia los conteos de por vida persistentes (`CounterState`, con `lifetime_key3` opcional y bandera `force_restore`) y devuelve el estado sincronizado del teclado. <!-- src: /protocol/opad.proto#CounterSyncRequest, /protocol/opad.proto#CounterSyncResponse -->
- **`DetectPinRequest` / `DetectPinResponse`**: Escanea GPIOs candidatos de interruptores durante `timeout_ms` (ignorando `exclude_gpio`) para detectar a qué pin está cableada `key_id`. <!-- src: /protocol/opad.proto#DetectPinRequest, /protocol/opad.proto#DetectPinResponse -->
- **`run_board_test` / `BoardTestResult`**: Ejecuta la prueba de PCB del carrier y módulo de entrada en `IDLE`: mide tensiones de ID de módulo (`id_mv`, `id_loaded_mv`, `reverse_probe_mv`), informa `boot_module`/`module` (`InputModule`), `cable_reversed`, GPIOs de teclas y máscaras de pull-up/pull-down/puentes, y en Hall Effect V3 incluye `HeAdcTest` (estado de reinicio de ADS7028, recorrido de registros, calibración de offset, barrido de velocidad SCLK con `0xA5A` y 3 ráfagas de canal de sensor). <!-- src: /protocol/opad.proto#BoardTestResult, /protocol/opad.proto#HeAdcTest -->
- **`KeyPress` / `KeyPressBatch`**: Agrupa marcas de tiempo de pulsaciones aceptadas de teclas de juego (`key`, `t_us` en microsegundos del teclado, más `now_us` del lote y conteo `dropped`) durante `HostStatus.playing` para el cálculo preciso del ritmo KPM. <!-- src: /protocol/opad.proto#KeyPressBatch, /firmware/main/protocol/protocol.c#protocol_send_key_presses -->
- **`reset_latency_stats`**: Borra el histograma de latencia flanco-a-HID y el contador de reportes diferidos en el teclado. <!-- src: /protocol/opad.proto#HostToDevice -->
- **`request_logs` / `LogEvent` / `LogEventBatch`**: Vacía eventos del búfer circular de diagnóstico (hasta 8 por lote) del teclado al daemon. <!-- src: /protocol/opad.proto#LogEventBatch, /firmware/main/protocol/protocol.c#protocol_send_log_batch -->

## Límites de búfer y campos en Nanopb

Los límites fijos de cadenas, bytes y arreglos repetidos se definen en `protocol/opad.options`: <!-- src: /protocol/opad.options -->

| Campo | Tamaño máximo / Límite | Tipo de asignación |
|---|---|---|
| `Hello.client_version` | 32 bytes | Cadena fija |
| `HelloAck.firmware_version` | 32 bytes | Cadena fija |
| `HelloAck.board_profile` | 32 bytes | Cadena fija |
| `HelloAck.device_id` | 32 bytes | Cadena fija |
| `HelloAck.owner_id` | 16 bytes | Arreglo fijo de bytes (`bytes`) |
| `HelloAck.running_partition` | 17 bytes | Cadena fija |
| `ClaimOwnership.owner_id` | 16 bytes | Arreglo fijo de bytes (`bytes`) |
| `ConfigAck.message` | 64 bytes | Cadena fija |
| `GameplayDisplayState.title` | 64 bytes | Cadena fija |
| `GameplayDisplayState.artist` | 64 bytes | Cadena fija |
| `GameplayDisplayState.difficulty` | 32 bytes | Cadena fija |
| `DataValue.text` | 64 bytes | Cadena fija |
| `DataUpdate.values` | 32 elementos | Mensaje repetido |
| `UiWidget.label` | 32 bytes | Cadena fija |
| `UiWidget.suffix` | 12 bytes | Cadena fija |
| `SetLayout.widgets` | 32 elementos | Mensaje repetido |
| `LayoutAck.message` | 64 bytes | Cadena fija |
| `PlayAnimation.grade` | 4 bytes | Cadena fija |
| `CounterState.device_id` | 32 bytes | Cadena fija |
| `CounterSyncResponse.message` | 64 bytes | Cadena fija |
| `LogEvent.tag` | 16 bytes | Cadena fija |
| `LogEvent.message` | 64 bytes | Cadena fija |
| `LogEventBatch.events` | 8 elementos | Mensaje repetido |
| `KeyPressBatch.presses` | 32 elementos | Mensaje repetido |
| `BoardTestResult.message` | 64 bytes | Cadena fija |
| `HeAdcTest.message` | 48 bytes | Cadena fija |
| `HeAdcTest.speeds` | 6 elementos | Mensaje repetido |
| `HeAdcTest.channels` | 3 elementos | Mensaje repetido |
| `HeCalibrationProgress.error_message` | 48 bytes | Cadena fija |

Estos límites estáticos permiten al firmware del ESP32-S3 codificar y decodificar mensajes protobuf sin asignación dinámica en el heap. <!-- src: /firmware/main/protocol/protocol.c#protocol_encode_device_message -->
