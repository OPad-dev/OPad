---
title: Documentación de OPad
description: OPad es un teclado de baja latencia para osu! sobre la Waveshare ESP32-S3-Touch-LCD-2, con software de escritorio opcional para Linux y Windows 10/11.
sidebar:
  order: 1
---

OPad es un teclado de dos teclas para osu! construido sobre la Waveshare ESP32-S3-Touch-LCD-2. Es un teclado USB a 1000 Hz, y su pantalla táctil es un tercer botón. <!-- src: /AGENTS.md#1. Product rules --> <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_config_desc -->

No necesitas ningún software para jugar: conecta el teclado y escribe como cualquier otro, en Linux y en Windows 10/11, sin controladores. La app de escritorio opcional (`opad-daemon`, `opad-gui` y `opadctl`) añade los extras: <!-- src: /AGENTS.md#1. Product rules -->

- datos en vivo del mapa y la partida de osu! en la pantalla del teclado, mediante el tosu incluido;
- un diseñador para las pantallas de reposo y de partida del teclado;
- contadores de pulsaciones de por vida guardados en el teclado y en tu PC;
- acciones por deslizamiento en la pantalla táctil;
- calibración Hall Effect y rapid trigger;
- actualizaciones de firmware por USB.

## Primeros pasos

1. [Qué necesitas](start/what-you-need.md): la placa, interruptores o PCBs V3, un cable y un sistema operativo compatible.
2. [Cableado de las teclas](start/wiring-the-keys.md): cablea a mano dos interruptores o monta el carrier y el módulo V3.
3. [Primera conexión](start/first-connection.md): conecta el teclado y comprueba que escribe.
4. [Instalar la app de escritorio](start/install-the-app.md): paquetes de Linux, AppImage y el instalador de Windows.

## Tareas habituales

- [Actualizar el firmware](guides/update-firmware.md)
- [Usar la pantalla táctil y los deslizamientos](guides/touchscreen-and-swipes.md)
- [Personalizar el diseño de la pantalla](guides/customize-screen-layout.md)
- [Calibrar interruptores Hall Effect](guides/calibrate-hall-effect.md)
- [Cambiar los pines de las teclas](guides/change-key-pins.md)
- [Usar OPad con tosu](guides/use-with-tosu.md)
- [Copia de seguridad y restauración](guides/backup-and-restore.md)
- [Desinstalar el software](guides/uninstall.md)

## Cuando algo va mal

- [Solución de problemas](troubleshooting.md): distribuciones de teclado, tosu en Linux, permisos USB, avisos de Windows.
- [Contadores, recuperación y desvinculación](recovery.md): cómo se reconcilian los contadores, cómo reflashear y cómo recuperar un teclado que no aparece. Nunca hace falta un botón BOOT o RESET.

## Referencia y contexto

- **Referencia:** [`opadctl`](reference/opadctl.md), [GUI](reference/gui.md), [ajustes del dispositivo](reference/device-settings.md), [pinout](reference/pinout.md), [protocolo serie](reference/serial-protocol.md), [IPC del daemon](reference/ipc.md), [archivos y rutas](reference/files-and-paths.md).
- **Cómo funciona:** [arquitectura](explanation/architecture.md), [latencia](explanation/latency.md), [la pantalla táctil como tercer botón](explanation/touchscreen-third-button.md), [versiones de la app y del teclado](explanation/app-and-pad-versions.md), [contadores y propiedad](explanation/counters-and-ownership.md), [actualizaciones y reversión](explanation/updates-and-rollback.md), [Hall Effect y rapid trigger](explanation/hall-effect.md).
- **Contribuir:** el [centro de desarrollo](develop/index.md) cubre la compilación, las pruebas, los detalles internos, el empaquetado y los lanzamientos. La [lista de verificación de pruebas](testing-checklist.md) recoge todas las comprobaciones de hardware y software.
