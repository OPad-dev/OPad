---
title: Shared Crates Reference
description: Technical catalog and architectural boundaries for the internal shared Rust crates comprising the desktop workspace.
category: reference
source_files:
  - /desktop/crates/opad-device/src/lib.rs
  - /desktop/crates/opad-ipc/src/lib.rs
  - /desktop/crates/opad-layout/src/lib.rs
  - /desktop/crates/opad-model/src/lib.rs
  - /desktop/crates/opad-protocol/src/lib.rs
  - /desktop/crates/opad-storage/src/lib.rs
  - /desktop/crates/opad-tosu/src/lib.rs
  - /desktop/crates/opad-ui-preview/src/lib.rs
  - /desktop/crates/opad-update/src/lib.rs
---

The desktop suite is modularized into nine focused internal crates located under `desktop/crates/`: <!-- src: /desktop/crates/opad-model/src/lib.rs -->

## Crate Catalog

### 1. `opad-model`
- **Purpose**: Foundational domain models and data structures shared across all binaries and crates. <!-- src: /desktop/crates/opad-model/src/lib.rs -->
- **Submodules**:
  - `paths`: Absolute filesystem path resolvers for data, logs, database, and bundled binaries (`paths.rs`). <!-- src: /desktop/crates/opad-model/src/paths.rs -->
  - `board_test`: Self-test report structures, module identification enum, and ADC telemetry.
  - `tap_rate`: Attempt stats, tap histograms, and KPM analytics models.
  - `swipe`: Touchscreen swipe action definitions and key bindings.
  - `result_anim`: Celebration splash animation identifiers.

### 2. `opad-protocol`
- **Purpose**: Pure-Rust Protocol Buffers wire encoding, decoding, and serial frame parsing. <!-- src: /desktop/crates/opad-protocol/src/lib.rs -->
- **Features**:
  - Compiles `protocol/opad.proto` using pure-Rust `protox`/`prost` without needing a system `protoc` compiler.
  - Implements marked framing (`0xAA, 0x55`) and checked framing (`0xAA, 0x5A`).
  - Standard CRC-16/CCITT-FALSE implementation (`crc16`). <!-- src: /desktop/crates/opad-protocol/src/lib.rs#crc16 -->

### 3. `opad-ipc`
- **Purpose**: Cross-process communication between `opad-daemon`, `opad-gui`, and `opadctl`. <!-- src: /desktop/crates/opad-ipc/src/lib.rs -->
- **Features**:
  - Unix domain stream sockets with user UID directory isolation (`0700/0600`).
  - Windows named pipes (`\\.\pipe\opad-ipc-<sid>`) hardened with user-only SDDL DACL security attributes.
  - Typed `IpcRequest` and `IpcResponse` JSON framing with 1 MiB request and 8 MiB response bounds. <!-- src: /desktop/crates/opad-ipc/src/lib.rs#MAX_REQUEST_FRAME_SIZE -->

### 4. `opad-storage`
- **Purpose**: SQLite database persistence layer for lifetime keypresses, device configurations, screen layouts, and installation identity. <!-- src: /desktop/crates/opad-storage/src/lib.rs -->
- **Features**:
  - WAL mode and foreign key constraints enabled on connection open.
  - Embedded schema migrations.
  - Monotonic counter reconciliation (`reconcile_counters`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#reconcile_counters -->
  - Write lockout enforcement during gameplay attempts (`StorageError::WritesBlocked`). <!-- src: /desktop/crates/opad-storage/src/lib.rs#StorageError::WritesBlocked -->

### 5. `opad-device`
- **Purpose**: Physical pad hardware discovery, serial CDC-ACM communication, and flashing engine. <!-- src: /desktop/crates/opad-device/src/lib.rs -->
- **Features**:
  - Device enumeration loop detecting pad connection within 2 seconds.
  - Direct hardware flashing routines (`flash::flash_firmware`, `recover`) interfacing with `espflash`.
  - ROM bootloader watchdog reset and partition table analysis.

### 6. `opad-layout`
- **Purpose**: Serialization, validation, and layout engine for the keypad's 320x240 LCD display. <!-- src: /desktop/crates/opad-layout/src/lib.rs -->
- **Features**:
  - Mirrors `ui_layout_t` and `ui_widget_t` definitions from firmware C headers verbatim.
  - Validates widget coordinate bounds, font bounds, and max widget counts (up to 32 widgets).

### 7. `opad-ui-preview`
- **Purpose**: Pixel-exact rendering of LCD screen layouts inside the desktop GUI designer. <!-- src: /desktop/crates/opad-ui-preview/src/lib.rs -->
- **Features**:
  - Recompiles the firmware's C LVGL graphics core on the host platform.
  - Renders screen layouts directly into RGBA image buffers without divergence from real hardware.

### 8. `opad-tosu`
- **Purpose**: Supervisor and WebSocket client for the bundled `tosu` memory scanner. <!-- src: /desktop/crates/opad-tosu/src/lib.rs -->
- **Features**:
  - Spawns and manages the `tosu` child process.
  - Subscribes to `ws://127.0.0.1:24050/websocket/v2`, parsing live song metadata, accuracy, combo, and UR.

### 9. `opad-update`
- **Purpose**: Self-update framework for desktop binaries, tosu, and pad firmware. <!-- src: /desktop/crates/opad-update/src/lib.rs -->
- **Features**:
  - Inspects GitHub release feeds for `stable` and `beta` channels.
  - Cryptographic checksum verification against signed release manifests.
  - Atomic executable replacement on disk.
