---
title: Documentación de OPad
description: Resumen de OPad, el teclado de baja latencia para osu! basado en ESP32-S3 y su suite de escritorio, con puntos de entrada para usuarios y desarrolladores.
sidebar:
  order: 1
---

OPad es un teclado USB HID de baja latencia a 1000 Hz para osu! construido sobre la placa Waveshare ESP32-S3-Touch-LCD-2, acompañado de una suite de escritorio opcional en Rust (`opad-daemon`, `opad-gui`, `opadctl`) para Linux y Windows 10/11. <!-- src: /README.md --> <!-- src: /AGENTS.md -->

El teclado funciona de inmediato como un teclado USB sin controladores y sin instalar ningún software. Añadir la suite de escritorio opcional habilita telemetría en vivo de osu! mediante `tosu`, un diseñador visual de diseño de pantalla, contadores de pulsaciones de por vida, acciones por deslizamiento en la pantalla táctil, calibración y ajuste de Rapid Trigger para interruptores Hall Effect, y actualizaciones de firmware por USB sin usar botones. <!-- src: /AGENTS.md --> <!-- src: /README.md -->

## Empieza aquí (Usar OPad)

- [Qué necesitas](start/what-you-need.md) — placa Waveshare, interruptores mecánicos o Hall Effect, PCBs OPad V3 y requisitos del sistema operativo.
- [Cableado de las teclas](start/wiring-the-keys.md) — cableado manual de interruptores al cabezal P2 o montaje del carrier y los módulos V3.
- [Primera conexión](start/first-connection.md) — conectar el teclado y verificar la escritura USB HID a 1000 Hz sin software.
- [Instalación de la suite de escritorio](start/install-the-app.md) — paquetes de Linux (`.deb`, `.rpm`, Arch, AppImage) e instalador de Windows.

## Tareas frecuentes

- [Actualización de firmware](guides/update-firmware.md) — actualización por USB desde `opad-gui` u `opadctl` con protección automática de reversión A/B.
- [Pantalla táctil y deslizamientos](guides/touchscreen-and-swipes.md) — Quick Retry dentro de los mapas y acciones de deslizamiento configurables fuera de los mapas.
- [Personalizar el diseño de pantalla](guides/customize-screen-layout.md) — crear, previsualizar, exportar e importar diseños personalizados `Idle` y `Playing` en el diseñador de pantalla.
- [Calibración de interruptores Hall Effect](guides/calibrate-hall-effect.md) — calibración guiada de 5 pulsaciones y ajuste de actuación / Rapid Trigger.
- [Cambio de pines de teclas](guides/change-key-pins.md) — reasignación de GPIOs para interruptores cableados a mano y uso de **Auto-Detect**.
- [Uso de OPad con tosu](guides/use-with-tosu.md) — configuración de telemetría en vivo de osu! y ajuste de `ptrace` en Linux.
- [Copia de seguridad y restauración](guides/backup-and-restore.md) — exportación e importación de la configuración y de los contadores de por vida.
- [Recuperación, reflasheo y desvinculación](recovery.md) — recuperación de un teclado que no responde sin botones BOOT/RESET, reconciliación de contadores y desvinculación.
- [Solución de problemas](troubleshooting.md) — distribuciones no QWERTY, `ptrace_scope` de Yama en Linux, reglas `udev`, Windows SmartScreen y recuperación.
- [Desinstalación de OPad](guides/uninstall.md) — eliminación limpia de la suite de escritorio en Linux y Windows.

## Referencia y arquitectura

- **Referencia**: [CLI `opadctl`](reference/opadctl.md), [Referencia de la GUI](reference/gui.md), [Ajustes del dispositivo](reference/device-settings.md), [Pinout](reference/pinout.md), [Protocolo serie](reference/serial-protocol.md), [IPC del daemon](reference/ipc.md), [Archivos y rutas](reference/files-and-paths.md).
- **Explicaciones**: [Arquitectura del sistema](explanation/architecture.md), [Ingeniería de latencia](explanation/latency.md), [La pantalla táctil como tercer botón](explanation/touchscreen-third-button.md), [Versiones de la app y del teclado](explanation/app-and-pad-versions.md), [Contadores y propiedad](explanation/counters-and-ownership.md), [Actualizaciones y reversión](explanation/updates-and-rollback.md), [Hall Effect y Rapid Trigger](explanation/hall-effect.md).

## Desarrollo de OPad

- [Centro de desarrollo](develop/index.md) — flujo de contribución, compilación del firmware (ESP-IDF v5.5.2) y escritorio (Rust), pruebas, detalles internos de firmware y escritorio, cambios de protocolo, hardware, empaquetado, lanzamientos y pruebas de latencia.
- [Lista de verificación de pruebas](testing-checklist.md) — matriz completa de verificación de hardware y software.
