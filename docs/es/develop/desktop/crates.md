---
title: Referencia de Crates Compartidas
description: Catálogo técnico y límites arquitectónicos para las crates Rust internas compartidas que componen el espacio de trabajo de escritorio.
---

La suite de escritorio está modularizada en nueve crates internas especializadas ubicadas bajo `desktop/crates/`: <!-- src: /desktop/crates/opad-model/src/lib.rs -->

## Crate Catalog

### 1. opad-model
- **Propósito**: Modelos de dominio fundamentales y estructuras de datos compartidas entre todos los binarios y crates. <!-- src: /desktop/crates/opad-model/src/lib.rs -->
- **Submódulos**:
  - `paths`: Resolutores de rutas absolutas en el sistema de archivos para datos, registros, base de datos y binarios empaquetados (`paths.rs`). <!-- src: /desktop/crates/opad-model/src/paths.rs -->
  - `board_test`: Estructuras de informes de autotest, enumeración de identificación de módulos y telemetría ADC.
  - `tap_rate`: Modelos de estadísticas de intentos, histogramas de pulsación y analíticas KPM.
  - `swipe`: Definiciones de acciones de deslizamiento táctil y enlaces de teclas.
  - `result_anim`: Identificadores de animaciones de celebración de resultados.

### 2. opad-protocol
- **Propósito**: Codificación, decodificación y análisis de tramas serie de Protocol Buffers sobre el cable en Rust puro. <!-- src: /desktop/crates/opad-protocol/src/lib.rs -->
- **Características**:
  - Compila `protocol/opad.proto` utilizando `protox`/`prost` en Rust puro sin requerir el compilador `protoc` del sistema.
  - Implementa enmarcado marcado (`0xAA, 0x55`) y enmarcado verificado (`0xAA, 0x5A`).
  - Implementación estándar de CRC-16/CCITT-FALSE (`crc16`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16 -->

### 3. opad-ipc
- **Propósito**: Comunicación entre procesos entre `opad-daemon`, `opad-gui` y `opadctl`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs -->
- **Características**:
  - Sockets de flujo de dominio Unix con aislamiento de directorio por UID de usuario (`0700/0600`).
  - Tuberías con nombre de Windows (`\\.\pipe\opad-ipc-<sid>`) blindadas con atributos de seguridad DACL SDDL exclusivos de usuario.
  - Enmarcado JSON tipado de `IpcRequest` e `IpcResponse` con límites de 1 MiB para peticiones y 8 MiB para respuestas. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->

### 4. opad-storage
- **Propósito**: Capa de persistencia en base de datos SQLite para pulsaciones de por vida, configuraciones de dispositivo, diseños de pantalla e identidad de instalación. <!-- src: /desktop/crates/opad-storage/src/lib.rs -->
- **Características**:
  - Modo WAL y restricciones de claves foráneas habilitados al abrir la conexión.
  - Migraciones de esquema integradas.
  - Reconciliación de contadores monotónicos (`reconcile_counters`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->
  - Aplicación de bloqueo de escritura durante las partidas activas (`StorageError::WritesBlocked`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#StorageError::WritesBlocked -->

### 5. opad-device
- **Propósito**: Detección de hardware físico del pad, comunicación serie CDC-ACM y motor de flasheo. <!-- src: /desktop/crates/opad-device/src/lib.rs -->
- **Características**:
  - Bucle de enumeración de dispositivos que detecta la conexión del pad en menos de 2 segundos.
  - Rutinas de flasheo de hardware directo (`flash::flash_firmware`, `recover`) interactuando con `espflash`.
  - Reinicio por watchdog en bootloader ROM y análisis de la tabla de particiones.

### 6. opad-layout
- **Propósito**: Serialización, validación y motor de diseño para la pantalla LCD de 320x240 del teclado. <!-- src: /desktop/crates/opad-layout/src/lib.rs -->
- **Características**:
  - Reproduce fielmente las definiciones `ui_layout_t` y `ui_widget_t` de las cabeceras en C del firmware.
  - Valida límites de coordenadas de widgets, límites de fuentes y número máximo de widgets (hasta 32 widgets).

### 7. opad-ui-preview
- **Propósito**: Renderizado con precisión de píxel de los diseños de pantalla LCD dentro del diseñador de la GUI de escritorio. <!-- src: /desktop/crates/opad-ui-preview/src/lib.rs -->
- **Características**:
  - Recompila el núcleo gráfico en C de LVGL del firmware en la plataforma anfitriona.
  - Renderiza los diseños de pantalla directamente en búferes de imagen RGBA sin divergencia con el hardware real.

### 8. opad-tosu
- **Propósito**: Supervisor y cliente WebSocket para el escáner de memoria empaquetado `tosu`. <!-- src: /desktop/crates/opad-tosu/src/lib.rs -->
- **Características**:
  - Inicia y gestiona el proceso hijo `tosu`.
  - Se suscribe a `ws://127.0.0.1:24050/websocket/v2`, analizando metadatos de canciones en vivo, precisión, combo y UR.

### 9. opad-update
- **Propósito**: Marco de autoactualización para binarios de escritorio, tosu y firmware del pad. <!-- src: /desktop/crates/opad-update/src/lib.rs -->
- **Características**:
  - Inspecciona los canales de publicación de GitHub para `stable` y `beta`.
  - Verificación criptográfica de sumas de comprobación frente a manifiestos de versión firmados.
  - Sustitución atómica de ejecutables en disco.
