---
title: Arquitectura de la herramienta CLI opadctl
description: Referencia de implementación de la utilidad de línea de comandos opadctl, análisis con clap, despacho de cliente IPC y flasheo/recuperación serie autónomo.
sidebar:
  order: 4
---

`opadctl` ([/desktop/cli/src/main.rs](/desktop/cli/src/main.rs)) es la CLI de OPad basada en Clap para consultas de estado, sincronización/reinicio de contadores, copias de seguridad JSON, monitorización de registros, inspección de latencia/ritmo de pulsación, pruebas de placa, configuración del entorno y flasheo/recuperación USB sin botones.

## Modos de ejecución

```text
+-------------------------------------------------------------+
|                      opadctl invocation                     |
+-------------------------------------------------------------+
                               |
        +----------------------+----------------------+----------------------+
        |                                             |                      |
        v                                             v                      v
[IPC Client Mode]                             [Direct Hardware Mode]  [Local Setup]
(status, sync, reset, export, import,         (flash, recover,        (setup)
 monitor, firmware-update, update, latency,    bootloader)
 stats, board-test, animation, restart)               |
        |                                             |
        v                                             v
Connects to opad-daemon via                   Asks daemon to release port
Unix socket / Windows named pipe              if running; flashes via espflash
```

1. **Modo cliente IPC**: Se conecta mediante `opad_ipc::connect_and_handshake`, envía tramas tipadas `IpcRequest` y formatea la salida de `IpcResponse`.
2. **Modo hardware directo (`flash`, `recover`, `bootloader`)**: Si `opad-daemon` está en ejecución, envía `IpcRequest::PrepareFlash` para que el daemon libere el puerto CDC, y luego ejecuta las rutinas `opad_device::flash` sobre el teclado (`303a:8415`) o el bootloader ROM del ESP32-S3 (`303a:1001`) usando `espflash` **4.6.0** fijado. Después envía `FinishFlash` (o `ResumeDevice` tras `bootloader`/fallo). Funciona de forma autónoma si el daemon está detenido.
3. **Modo configuración local (`setup`)**: Comprueba localmente `70-opad.rules` y `ptrace_scope` / `cap_sys_ptrace` de Yama en Linux sin necesitar `opad-daemon` (o informa de los controladores integrados en Windows).

## Subcomandos (enum `Commands`)

Los 17 subcomandos en [/desktop/cli/src/main.rs](/desktop/cli/src/main.rs):
- `status`: Imprime el modo del daemon, info del teclado, ranura OTA activa, módulo de entrada y calibración HE, contadores, propiedad, estado de `tosu`, latencia (`p50`/`p99.9`/`max`) y configuración.
- `sync`: Envía `IpcRequest::ForceSync`.
- `reset [--yes]`: Reinicio destructivo de contadores con incremento de generación (aborta sin `--yes`).
- `export <FILE>` / `import [--yes] <FILE>`: Exporta o previsualiza e importa archivos `JsonBackup`.
- `monitor [-l <LIMIT>] [-f] [--level <LEVEL>] [--source <SOURCE>]`: Transmite registros de daemon/teclado/programa/`tosu` (sondea cada 500 ms con `-f`).
- `flash [--port <PORT>] [--full] <FIRMWARE>`: Actualización OTA en la ranura inactiva o flasheo completo de recuperación `--full`.
- `recover [--timeout <SECONDS>] <FIRMWARE>`: Captura el bootloader ROM del ESP32-S3 al reconectar el cable y escribe el conjunto completo de flash (sin botones BOOT ni RESET).
- `bootloader [--port <PORT>]`: Reinicia el teclado en modo descarga ROM (`303a:1001`) y lo deja ahí.
- `firmware-update [--yes]`: Comprueba el manifiesto firmado de lanzamiento e instala la actualización de firmware con consentimiento explícito.
- `update [--channel <CHANNEL>] [--check] [--install]`: Muestra el estado de actualización de la aplicación, cambia el canal (`stable`/`beta`), comprueba el feed (esperando hasta 120 s) o inicia la instalación de una actualización disponible mediante `IpcRequest::InstallUpdate`.
- `latency [--reset]`: Imprime o reinicia la latencia tecla-a-HID medida en el dispositivo (`p50`, `p99`, `p99.9`, `max`, conteo diferido).
- `stats [--period <DAYS>]`: Imprime ritmos de pulsación KPM de intentos e historial (y opcionalmente actualiza el periodo histórico, `0` = todo el tiempo, máximo `3650`).
- `board-test [--fix-pins] [--magnet]`: Ejecuta pruebas de PCB del carrier/módulo (ID de módulo, orientación FFC, líneas de teclas, cortocircuitos, ADS7028/sensores de HE V3).
- `animation <NAME> [GRADE]`: Previsualiza una animación de resultado (`pop`, `drop`, `burst`, ID o `none`) para calificación `SS`/`S`/`A`/`B`/`C`/`D`.
- `restart`: Vuelca contadores y reinicia el teclado en cuanto quede en reposo.
- `setup`: Verifica permisos de plataforma (`udev` + `ptrace` en Linux).
