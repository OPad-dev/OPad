---
title: Visión General de la Suite de Escritorio
description: Visión arquitectónica general del espacio de trabajo Rust de escritorio, modelo de procesos, transporte IPC multiplataforma y límites internos entre crates.
---

La suite de escritorio de OPad está implementada en Rust (edición 2021) como un espacio de trabajo de Cargo que contiene tres binarios de aplicación de nivel superior y nueve crates de biblioteca modulares internas. Proporciona soporte multiplataforma en Linux (X11 / Wayland) y Windows 10/11. <!-- src: /desktop/Cargo.toml -->

## Process Model & Separation of Concerns

La suite de escritorio delega responsabilidades a través de procesos independientes: <!-- src: /desktop/daemon/src/main.rs -->

```
+--------------------+            IPC             +--------------------+
|     opad-gui       | <------------------------> |    opad-daemon     |
| (iced / GUI / Tray)|  (Socket Unix / Tubería)   | (Runtime Tokio)    |
+--------------------+                            +--------------------+
                                                             |
+--------------------+            IPC                        | Serie CDC ACM
|      opadctl       | <-------------------------------------+
| (Utilidad CLI Clap)|                                       |
+--------------------+                                       v
                                                  +--------------------+
                                                  |     Pad Físico     |
                                                  |  (Disp. ESP32-S3)  |
                                                  +--------------------+
```

### 1. The Daemon (opad-daemon)
Un servicio en segundo plano sin interfaz que se ejecuta sobre Tokio. Mantiene acceso exclusivo al puerto serie CDC-ACM del teclado, ejecuta la cola del actor del dispositivo, supervisa `tosu`, persiste contadores en SQLite y atiende solicitudes IPC. <!-- src: /desktop/daemon/src/main.rs -->

### 2. The Graphical Interface (opad-gui)
Una aplicación de escritorio basada en iced. Renderiza analíticas de panel en vivo, proporciona el diseñador visual de pantallas, ajusta los umbrales de Rapid Trigger de Hall Effect y muestra diagnósticos en tiempo real. Si se inicia mientras `opad-daemon` está desconectado, inicia automáticamente el daemon. <!-- src: /desktop/gui/src/main.rs -->

### 3. The CLI (opadctl)
Una herramienta de línea de comandos construida con Clap. Proporciona consultas de estado, sincronización de contadores, importación/exportación de copias de seguridad y rutinas de flasheo directo por serie (`opadctl flash` y `opadctl recover`). <!-- src: /desktop/cli/src/main.rs -->

## Workspace Crates Topology

Las bibliotecas internas están desacopladas en crates especializadas (`desktop/crates/`): <!-- src: /desktop/Cargo.toml -->

- **`opad-model`**: Modelos de datos principales, catálogos de ajustes de dispositivos, mapeos de pines de teclas, modelos de tasa de pulsación y funciones auxiliares de resolución de rutas.
- **`opad-protocol`**: Códecs de protobuf en Rust puro (`prost`/`protox`), analizadores de tramas, marcas mágicas y cálculo de CRC-16.
- **`opad-ipc`**: Capa de comunicación entre procesos, sockets de dominio Unix, tuberías con nombre de Windows con seguridad DACL y enmarcado de mensajes JSON.
- **`opad-storage`**: Capa de persistencia SQLite, migraciones de esquema, bloqueos de escritura durante el juego y reconciliación de contadores.
- **`opad-device`**: Gestor de comunicación serie USB, bucles de descubrimiento de dispositivos y rutinas de flasheo serie.
- **`opad-tosu`**: Supervisor que gestiona el lector de memoria y cliente WebSocket integrado de `tosu`.
- **`opad-layout`**: Modelos de diseño de widgets para LCD, definiciones de fuentes y lógica de empaquetado/desempaquetado serie.
- **`opad-ui-preview`**: Motor de pantalla LVGL del firmware compilado para el escritorio para ofrecer previsualizaciones de diseño con precisión de píxel.
- **`opad-update`**: Motor de autoactualización que gestiona comprobaciones de versiones en GitHub, firmas y descompresión de binarios.
