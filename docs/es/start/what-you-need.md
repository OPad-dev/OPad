---
title: Qué necesitas
description: Un OPad, un cable USB-C de datos y un ordenador con Linux o Windows 10/11. La app de escritorio es opcional.
sidebar:
  order: 1
---

- **Tu OPad.** Viene montado, con tres teclas (mecánicas o Hall Effect, según la versión que hayas elegido; osu! se juega con dos y la tercera está para tu estilo de juego), además de una pantalla táctil que puedes tocar para hacer Quick Retry durante un mapa. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) --> <!-- src: /hardware/pcb/V3/README.md --> <!-- src: /AGENTS.md#1. Product rules -->
- **Un cable USB-C de datos.** El teclado trae uno. Si usas otro, asegúrate de que transmite datos: un cable solo de carga alimenta el teclado, pero tu PC nunca lo ve como teclado. <!-- src: /docs/launch-checklist.md#Decisions (owner, 2026-11-21) -->
- **Un ordenador con Linux o Windows 10/11 (x64).** El teclado usa los controladores que tu sistema ya trae (`usbhid` y `cdc_acm` en Linux, `hidclass.sys` y `usbser.sys` en Windows), así que nunca instalas un controlador. <!-- src: /README.md --> <!-- src: /packaging/windows/installer.iss -->

La app de escritorio es opcional. Sin ella, el teclado sigue siendo un teclado completo a 1000 Hz y su pantalla táctil envía Quick Retry. [Instalar la app de escritorio](install-the-app.md) enumera lo que añade la app y para qué distribuciones de Linux se compilan sus paquetes. <!-- src: /AGENTS.md#1. Product rules -->

¿Vas a montar tu propio teclado a partir de una placa Waveshare? Empieza por [Piezas y placas](../build/parts.md).

## Siguiente paso

Conecta el teclado: [Primera conexión](first-connection.md).
