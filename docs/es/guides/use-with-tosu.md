---
title: Uso con tosu
description: Cómo se integra OPad con tosu para la telemetría en vivo de mapas de osu!, configuración de ptrace en Linux, binarios personalizados de tosu y verificación de estado.
sidebar:
  order: 6
---

OPad incluye `tosu` (v4.26.2, LGPL-3.0) para leer el estado en vivo de `osu!` / `osu!lazer` y transmitir metadatos del mapa, precisión, PP, combo, unstable rate (UR) y ritmo de pulsación a la pantalla LCD del teclado. <!-- src: /desktop/crates/opad-tosu/src/lib.rs --> <!-- src: /Makefile -->

## Qué necesitas

- La suite de escritorio de OPad instalada (`opad-daemon` en ejecución) y tu OPad conectado por USB.
- `osu!` (stable u `osu!lazer`) ejecutándose en Linux o Windows 10/11.

## Cómo funciona la integración con tosu

1. **Supervisión automática**: Cuando `opad-daemon` arranca, su supervisor de `tosu` comprueba si ya hay algo escuchando en `127.0.0.1:24050`. Si no, lanza `tosu` automáticamente (buscando `$OPAD_TOSU_PATH`, `~/.local/opt/tosu/tosu`, `tosu` en `$PATH` y finalmente la copia incluida en último lugar, de modo que un `tosu` instalado por el usuario siempre tiene prioridad). Reinicia `tosu` con retroceso exponencial (de 5 s hasta 60 s) si finaliza, y desactiva el autoactualizador propio de `tosu` y la apertura automática del panel en el navegador. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#spawn_tosu_supervisor -->
2. **Conexión WebSocket**: `opad-daemon` se conecta a `ws://127.0.0.1:24050/websocket/v2` para el estado del mapa y la partida, y a `/websocket/v2/precise` para los contadores K1/K2 de osu!. (Las URLs antiguas `/ws` almacenadas se normalizan a `/websocket/v2`.) <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#follow_keys -->
3. **Actualizaciones USB limitadas en frecuencia**: `DataSync` envía al teclado solo los valores modificados, limitados a la **GAMEPLAY REFRESH RATE** configurada (1–30 Hz, 10 Hz por defecto, ajustable en **Settings** -> **🖥 Display & Game**) durante la partida y a 2 Hz (`500 ms`) en reposo. <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing --> <!-- src: /desktop/gui/src/pages.rs#settings -->
4. **Conciencia de la pantalla táctil**: Mientras `tosu` está conectado, el teclado sabe cuándo estás dentro de un mapa (`Playing`) frente a los menús, habilitando Quick Retry durante los mapas y acciones de deslizamiento fuera de ellos. Sin `tosu`, cualquier toque actúa como Quick Retry. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/main/input/touch_retry.c -->

## Configuración en Linux (permisos `ptrace`)

Con Yama en Linux (`/proc/sys/kernel/yama/ptrace_scope > 0`), los procesos sin privilegios no pueden inspeccionar la memoria de procesos no emparentados como `osu!`: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

- **Paquetes `.deb` y `.rpm`**: El script postinstalación del paquete ejecuta automáticamente `setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu`, de modo que `tosu` funciona directamente sin rebajar el `ptrace_scope` global del sistema. <!-- src: /packaging/linux/deb/postinst -->
- **AppImage y Arch (`PKGBUILD`)**: Las capacidades de archivo no funcionan en AppImage (montaje FUSE `nosuid`) y no se aplican al binario `node` del sistema en Arch. En esas instalaciones, baja `ptrace_scope` a `0` (consulta [Solución de problemas §2](../troubleshooting.md)): <!-- src: /packaging/linux/arch/PKGBUILD --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->
  ```bash
  echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
  ```
  Para hacerlo persistente entre reinicios, configura `kernel.yama.ptrace_scope = 0` en `/etc/sysctl.d/10-ptrace.conf` y ejecuta `sudo sysctl --system`.

Ejecuta `opadctl setup` para comprobar el estado exacto de `ptrace_scope` y las capacidades. <!-- src: /desktop/cli/src/main.rs#run_setup -->

## Configuración en Windows

En Windows 10 y 11, `tosu.exe` funciona directamente en cuanto `opad-daemon` está en marcha y `osu!` está abierto. Si la telemetría no aparece nunca, comprueba si el antivirus ha puesto en cuarentena `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe` (consulta [Solución de problemas §4](../troubleshooting.md)). <!-- src: /packaging/windows/installer.iss -->

## Usar tu propio binario de `tosu`

Bajo LGPL-3.0 puedes reemplazar el `tosu` incluido en cualquier momento: <!-- src: /desktop/gui/src/pages.rs#tosu_settings -->
- En `opad-gui`: abre **Settings** -> **ℹ About & tosu** y haz clic en **Browse external tosu...** (haz clic en **Use bundled** para volver al incluido).
- O define la variable de entorno `OPAD_TOSU_PATH` con la ruta a tu ejecutable de `tosu`, o ejecuta tu propio `tosu` en el puerto `24050` antes de iniciar `opad-daemon`. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

## Cómo comprobarlo

1. En `opad-gui`, abre **Settings** -> **ℹ About & tosu** y confirma que **Status:** muestre **Connected (streaming telemetry)** (y revisa **Binary source:**). <!-- src: /desktop/gui/src/pages.rs#tosu_settings -->
2. O ejecuta `opadctl status` y confirma que `tosu (osu!)` esté conectado. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
3. Inicia un mapa en `osu!`: la pantalla LCD del teclado cambia al diseño de partida con estadísticas del mapa en vivo.

## Qué hacer si no funcionó

- **`tosu` en Linux conecta pero muestra estadísticas vacías**: Ejecuta `opadctl setup` o abre **Diagnostics** en `opad-gui` y sigue las instrucciones de `ptrace_scope` de Yama en [Solución de problemas §2](../troubleshooting.md).
- **Revisa los registros de `tosu`**: Ejecuta `opadctl monitor --source tosu` o inspecciona `<state_dir>/tosu.log`. <!-- src: /desktop/cli/src/main.rs#Commands::Monitor --> <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path -->
