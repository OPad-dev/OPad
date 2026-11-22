---
title: Visión general de la suite de escritorio
description: Visión arquitectónica de alto nivel del espacio de trabajo de escritorio en Rust, modelo de procesos, transporte IPC multiplataforma y límites entre crates internos.
sidebar:
  order: 1
---

La suite de escritorio de OPad es un espacio de trabajo Cargo en Rust (edición 2021) ([/desktop/Cargo.toml](/desktop/Cargo.toml)) que contiene tres binarios (`opad-daemon`, `opad-gui`, `opadctl`) y nueve crates de librería internos compatibles con Linux (X11/Wayland) y Windows 10/11.

## Modelo de procesos y separación de responsabilidades

```text
+--------------------+            IPC             +--------------------+
|     opad-gui       | <------------------------> |    opad-daemon     |
| (iced / GUI / Tray)|   (Unix Socket / Pipe)     | (Tokio Runtime)    |
+--------------------+                            +--------------------+
                                                             |
+--------------------+            IPC                        | USB CDC-ACM
|      opadctl       | <-------------------------------------+
| (Clap CLI Utility) |                                       |
+--------------------+                                       v
                                                  +--------------------+
                                                  |    Physical Pad    |
                                                  |  (ESP32-S3 Device) |
                                                  +--------------------+
```

1. **`opad-daemon`** ([/desktop/daemon/src/main.rs](/desktop/daemon/src/main.rs)): Servicio Tokio en segundo plano sin interfaz gráfica que mantiene el puerto serie CDC-ACM del teclado, ejecuta el actor ordenado de dispositivo, supervisa `tosu`, persiste el estado en SQLite (`opad.db`) y atiende IPC local.
2. **`opad-gui`** ([/desktop/gui/src/main.rs](/desktop/gui/src/main.rs)): Aplicación gráfica basada en `iced` y applet en la bandeja del sistema (`--tray`, `--page <name>`). Arranca `opad-daemon` automáticamente si está detenido.
3. **`opadctl`** ([/desktop/cli/src/main.rs](/desktop/cli/src/main.rs)): CLI basada en Clap que se comunica por IPC para gestión/diagnóstico y directamente por serie USB para `flash`, `recover` y `bootloader`.

## Crates del espacio de trabajo (`desktop/crates/`)

- **`opad-model`** ([/desktop/crates/opad-model/src/lib.rs](/desktop/crates/opad-model/src/lib.rs)): Tipos de dominio (`DeviceInfo`, `DeviceConfig`, `CounterState`, `JsonBackup`), rutas, acciones de deslizamiento, cálculo de ritmo de pulsación, evaluación de pruebas de placa y animaciones de resultados.
- **`opad-protocol`** ([/desktop/crates/opad-protocol/src/lib.rs](/desktop/crates/opad-protocol/src/lib.rs)): Protobuf en Rust puro (`protox`/`prost`), entramado Marked/Checked y CRC-16/CCITT-FALSE.
- **`opad-ipc`** ([/desktop/crates/opad-ipc/src/lib.rs](/desktop/crates/opad-ipc/src/lib.rs)): JSON delimitado por longitud sobre sockets de dominio Unix (`0700`/`0600`) o Named Pipes de Windows (DACL SDDL protegida solo para el usuario).
- **`opad-storage`** ([/desktop/crates/opad-storage/src/lib.rs](/desktop/crates/opad-storage/src/lib.rs)): SQLite (WAL, esquema v19), guardia de escrituras durante partida (`StorageError::WritesBlocked`) y reconciliación de contadores.
- **`opad-device`** ([/desktop/crates/opad-device/src/lib.rs](/desktop/crates/opad-device/src/lib.rs)): Descubrimiento USB CDC, apretón de manos/negociación de entramado y flasheo/recuperación basados en `espflash`.
- **`opad-tosu`** ([/desktop/crates/opad-tosu/src/lib.rs](/desktop/crates/opad-tosu/src/lib.rs)): Supervisor de proceso hijo `tosu`, cliente WebSocket v2/precise y comprobaciones Yama `ptrace` en Linux.
- **`opad-layout`** ([/desktop/crates/opad-layout/src/lib.rs](/desktop/crates/opad-layout/src/lib.rs)): Validación de diseños `320x240` y empaquetado binario equivalente a `ui_layout_t` del firmware.
- **`opad-ui-preview`** ([/desktop/crates/opad-ui-preview/src/lib.rs](/desktop/crates/opad-ui-preview/src/lib.rs)): Renderizador de vista previa LVGL del firmware compilado en el anfitrión para el diseñador de pantalla de la GUI.
- **`opad-update`** ([/desktop/crates/opad-update/src/lib.rs](/desktop/crates/opad-update/src/lib.rs)): Verificación de feeds de actualización firmados (minisign Ed25519 + SHA-256), políticas por origen de instalación, reemplazos atómicos de binarios y `opad-feed`.
