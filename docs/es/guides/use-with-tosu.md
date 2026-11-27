---
title: Usar OPad con tosu
description: Cómo usa OPad tosu para mostrar datos de osu! en vivo en la pantalla del teclado, qué configurar en Linux, cómo usar tu propio tosu y cómo comprobar que funciona.
sidebar:
  order: 6
---

[tosu](https://github.com/tosuapp/tosu) lee la memoria de osu! y sirve el mapa y la partida actuales por un WebSocket local. OPad incluye tosu v4.26.2 (LGPL-3.0) y lo usa para: <!-- src: /Makefile --> <!-- src: /desktop/gui/src/settings.rs#about -->

- mostrar datos del mapa y de la partida en la pantalla del teclado (título, estrellas, precisión, PP, combo, UR, recuento de aciertos y más; consulta [Personalizar el diseño de la pantalla](customize-screen-layout.md));
- cambiar el teclado a su pantalla **Playing** durante un mapa; <!-- src: /firmware/main/ui/ui_port.c#pad_timer_cb -->
- indicar a la pantalla táctil cuándo estás en un mapa, para que un dedo apoyado sea Quick Retry ahí y los deslizamientos funcionen fuera de él (consulta [Pantalla táctil y deslizamientos](touchscreen-and-swipes.md)). <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->

Tus teclas nunca dependen de tosu: el teclado escribe igual con él o sin él. <!-- src: /AGENTS.md#1. Product rules -->

## Qué necesitas

- La app de escritorio de OPad instalada, con el teclado conectado.
- osu! (stable o lazer) en ejecución.

No hay nada que arrancar a mano: el daemon ejecuta tosu por ti.

## Cómo funciona

1. **El daemon arranca tosu.** Si todavía no hay nada escuchando en el puerto de tosu (`127.0.0.1:24050`), el daemon arranca tosu y lo reinicia si termina (tras 5 segundos, duplicando el tiempo hasta 60). Desactiva el actualizador propio de tosu y su costumbre de abrir un panel en el navegador. Si ya ejecutas tú un tosu, el daemon usa ese. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#spawn_tosu_supervisor --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#INITIAL_BACKOFF -->
2. **El daemon lee los datos de tosu** desde `ws://127.0.0.1:24050/websocket/v2`, y los recuentos de teclas del propio osu! desde `/websocket/v2/precise`. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#precise_endpoint -->
3. **El daemon solo envía al teclado lo que ha cambiado.** Durante un mapa envía al ritmo fijado en **Screen** → **SCREEN UPDATES DURING A MAP** (1–30 Hz, 10 Hz de forma predeterminada), y fuera de un mapa dos veces por segundo. <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing --> <!-- src: /desktop/daemon/src/telemetry.rs#FLUSH_INTERVAL_IDLE --> <!-- src: /desktop/gui/src/screen.rs#display -->

El daemon busca tosu en este orden y usa el primero que encuentra: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

1. el archivo de la variable de entorno `OPAD_TOSU_PATH`;
2. `~/.local/opt/tosu/tosu` (`tosu.exe` en Windows);
3. `tosu` en tu `PATH`;
4. la copia incluida.

Así que un tosu que hayas instalado tú siempre tiene prioridad sobre el incluido.

## Linux: deja que tosu lea osu!

En Linux, el ajuste `ptrace_scope` de Yama impide que un programa lea la memoria de otro siempre que está por encima de 0, así que tosu necesita permiso. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

- **Los paquetes `.deb`, `.rpm` y el paquete de Arch de las versiones publicadas** dan al tosu incluido la capacidad `cap_sys_ptrace` al instalarse. No hay que hacer nada. <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/opad-bin.install -->
- **La AppImage y el paquete de Arch compilado desde el código fuente** no pueden llevar esa capacidad. En ellos, baja `ptrace_scope` a 0. <!-- src: /packaging/linux/arch/PKGBUILD -->

Para ver qué caso es el tuyo, ejecuta:

```bash
opadctl doctor
```

Muestra tu `ptrace_scope` y el comando exacto que lo arregla. [Solución de problemas §2](../troubleshooting.md#2-linux-tosu-no-puede-leer-osu-yama-ptrace_scope) explica las dos soluciones. <!-- src: /desktop/cli/src/main.rs#run_doctor -->

## Windows

tosu funciona sin configurar nada. Si los datos en vivo nunca aparecen, comprueba que tu antivirus no ha puesto en cuarentena `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`; consulta [Solución de problemas §4](../troubleshooting.md#4-windows-smartscreen-y-antivirus). <!-- src: /packaging/windows/installer.iss -->

## Usa tu propio tosu

tosu es LGPL-3.0, así que puedes sustituir la copia incluida por tu propia compilación. Elige la forma que mejor te venga: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

- **Ejecuta antes tu propio tosu.** Cuando algo ya escucha en el puerto 24050, el daemon no arranca ninguno y lee del tuyo. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#spawn_tosu_supervisor -->
- **Instálalo en `~/.local/opt/tosu/` o ponlo en tu `PATH`.** El daemon lo prefiere a la copia incluida.
- **Define `OPAD_TOSU_PATH`** con tu ejecutable de tosu en el entorno del daemon.
- **En la app:** **Settings** → **osu! connection** → **Choose another tosu…** (y **Use the one that comes with OPad** para volver). La app recuerda el archivo y lo usa cuando arranca tosu o el propio daemon. Un daemon que arranca al iniciar sesión (systemd o la entrada Run de Windows) no ve esta elección; para él, usa una de las formas anteriores. <!-- src: /desktop/gui/src/settings.rs#osu_connection --> <!-- src: /desktop/gui/src/main.rs#App --> <!-- src: /desktop/gui/src/platform_linux.rs#start_daemon -->

## Comprueba que funciona

1. Ejecuta `opadctl status` y busca `tosu (osu!):      Active`. <!-- src: /desktop/cli/src/main.rs#run -->
2. O abre **Settings** → **osu! connection** en la app: debería decir `Connected`, e **In use** indica qué tosu se está usando. <!-- src: /desktop/gui/src/settings.rs#osu_connection -->
3. Empieza un mapa en osu!. El teclado cambia a su pantalla Playing con datos en vivo.

## Si no ha funcionado

- **Linux: tosu está conectado, pero el teclado no muestra datos del mapa:** tosu no puede leer la memoria de osu!. Ejecuta `opadctl doctor` y sigue [Solución de problemas §2](../troubleshooting.md#2-linux-tosu-no-puede-leer-osu-yama-ptrace_scope).
- **tosu no arranca o se reinicia una y otra vez:** lee su registro con `opadctl logs --source tosu` o abre `tosu.log` (en `~/.local/state/opad/` en Linux, `%APPDATA%\opad\` en Windows). <!-- src: /desktop/cli/src/main.rs#LogsArgs --> <!-- src: /desktop/crates/opad-model/src/paths.rs#tosu_log_path -->
