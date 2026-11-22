---
title: Arquitectura de la Herramienta CLI opadctl
description: Referencia de implementación para la utilidad de línea de comandos opadctl, análisis de argumentos con clap, despacho de cliente IPC y lógica de flasheo serie autónomo.
---

El binario `opadctl` es la herramienta de diagnóstico y gestión por línea de comandos para OPad. Proporciona gestión del dispositivo automatizable mediante scripts, reconciliación de contadores, operaciones de copia de seguridad, monitorización de registros y rutinas de flasheo directo de firmware. <!-- src: /desktop/cli/src/main.rs -->

## Dual Mode Execution Architecture

`opadctl` se ejecuta en dos modos operativos diferenciados según el subcomando invocado: <!-- src: /desktop/cli/src/main.rs -->

```
+-------------------------------------------------------------+
|                     Invocación de opadctl                   |
+-------------------------------------------------------------+
                               |
        +----------------------+----------------------+
        |                                             |
        v                                             v
[Modo Cliente IPC]                            [Modo Hardware Directo]
(Status, Sync, Reset, Config,                 (Flash, Recover, Bootloader)
 Backup, Monitor, Board-Test)                         |
        |                                             |
        v                                             v
Se comunica con `opad-daemon`                 Omite el daemon, accede directamente
vía Socket Unix o Tubería                     al USB CDC / bootloader ROM
```

### 1. IPC Client Mode (Daemon-Mediated)
Para los comandos que consultan o modifican el estado (`status`, `sync`, `reset`, `config`, `backup`, `monitor`, `board-test`): <!-- src: /desktop/cli/src/main.rs -->
- Se conecta al daemon local mediante `opad_ipc::connect_and_handshake`.
- Aplica la seguridad de sesión de usuario (0700/0600 en Linux, DACL SDDL restringida a usuario en Windows).
- Empaqueta los parámetros en tramas JSON tipadas `IpcRequest` y espera la respuesta `IpcResponse`.
- Renderiza salidas tabulares formateadas en terminal, cargas JSON o flujos de seguimiento en vivo.

### 2. Direct Hardware Mode (Daemon-Independent)
Para comandos de flasheo de firmware y recuperación del chip (`flash`, `recover`, `bootloader`): <!-- src: /desktop/cli/src/main.rs -->
- Si `opad-daemon` está en ejecución, `opadctl` transmite `IpcRequest::PrepareFlash` para ordenar educadamente al daemon que libere el puerto serie CDC.
- Si `opad-daemon` está desconectado o no responde, `opadctl` continúa de forma autónoma, tomando directamente el puerto serie del dispositivo.
- Se comunica directamente con el bootloader ROM del ESP32-S3 (`303a:1001`) mediante rutinas de `opad-device::flash`.
- Ejecuta el análisis de la tabla de particiones, síntesis de `otadata` de prueba y reinicios por watchdog RTC.

## Command Catalog Implementation

Los subcomandos de la CLI se analizan mediante macros derive de `clap` (enumeración `Commands` en `desktop/cli/src/main.rs`): <!-- src: /desktop/cli/src/main.rs#Commands -->

- **`status`**: Consulta el estado del daemon, conexión de hardware, ID de perfil, modo de juego y contadores más recientes.
- **`sync`**: Fuerza una sincronización bidireccional inmediata entre el almacenamiento SQLite del escritorio y la NVS del pad.
- **`reset --yes`**: Ejecuta un incremento destructivo de generación y restablecimiento de contadores. Requiere la opción de confirmación explícita `--yes`.
- **`config get` / `config set`**: Lee o modifica ajustes individuales del dispositivo (ms de debounce, GPIOs de teclas, brillo de pantalla).
- **`backup export` / `backup import` / `backup preview`**: Serializa y deserializa archivos `JsonBackup` con protección contra retroceso de contadores.
- **`monitor --follow`**: Transmite eventos de registro de diagnóstico en tiempo real desde los búferes circulares del daemon y del pad.
- **`board-test`**: Ejecuta rutinas de autotest de la placa (I2C, ID de pantalla, táctil, canales ADC).
- **`flash`**: Flashea el binario de firmware en la partición OTA inactiva con protección de reversión automática.
- **`recover`**: Recupera un pad que no responde mediante una ventana de reconexión de cable sin requerir pulsaciones del botón físico BOOT.
