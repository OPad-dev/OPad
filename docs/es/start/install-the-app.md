---
title: Instalar la app de escritorio
description: Instala la app de escritorio opcional de OPad (opad-daemon, opad-gui, opadctl y el tosu incluido) en Linux o Windows 10/11.
sidebar:
  order: 4
---

La app de escritorio es opcional: el teclado escribe sin ella. Instálala para tener datos de osu! en vivo en la pantalla del teclado, el diseñador de pantalla, contadores de por vida en tu PC, acciones por deslizamiento, ajuste Hall Effect y actualizaciones de firmware. <!-- src: /AGENTS.md#1. Product rules -->

Tiene cuatro partes: <!-- src: /packaging/windows/installer.iss --> <!-- src: /desktop/gui/Cargo.toml -->

| Programa | Qué hace |
|---|---|
| `opad-daemon` | Se ejecuta en segundo plano. Habla con el teclado por su puerto serie, guarda tus contadores y ajustes y ejecuta tosu. |
| `opad-gui` | La ventana de OPad y el icono de la bandeja: ajustes, Designer, Stats, Device y Diagnostics. |
| `opadctl` | Las mismas funciones desde un terminal, además del flasheo y la recuperación. Consulta la [referencia de `opadctl`](../reference/opadctl.md). |
| tosu | Lee la memoria de osu! para obtener datos del mapa y de la partida. Viene incluido (LGPL-3.0); puedes usar el tuyo, consulta [Usar OPad con tosu](../guides/use-with-tosu.md). |

## Descarga

Cada versión en la página de [GitHub Releases](https://github.com/OPad-dev/OPad/releases) indica, al principio de sus notas, qué archivo es para tu sistema: <!-- src: /scripts/release/release_notes.sh -->

| Sistema | Archivo |
|---|---|
| Windows 10 u 11 | `OPad-Windows-Setup-<version>.exe` |
| Ubuntu o Debian | `OPad-Linux-Debian-Ubuntu-<version>.deb` |
| Fedora, RHEL, AlmaLinux, Rocky | `OPad-Linux-Fedora-<version>.rpm` |
| Arch Linux | `OPad-Linux-Arch-<version>.pkg.tar.zst` |
| Cualquier otro Linux | `OPad-Linux-<version>.AppImage` |

## Linux

### Debian y Ubuntu

```bash
sudo apt install ./OPad-Linux-Debian-Ubuntu-<version>.deb
```

### Fedora, RHEL, AlmaLinux y Rocky

```bash
sudo dnf install ./OPad-Linux-Fedora-<version>.rpm
```

### Arch Linux

```bash
sudo pacman -U ./OPad-Linux-Arch-<version>.pkg.tar.zst
```

Esto instala el paquete `opad-bin`: los mismos binarios y el mismo tosu incluido que el `.deb`. <!-- src: /packaging/linux/arch/PKGBUILD-bin.in --> <!-- src: /.github/workflows/release-app.yml -->

### Qué configuran los paquetes

Los paquetes `.deb`, `.rpm` y de Arch instalan todos lo mismo: <!-- src: /desktop/gui/Cargo.toml#package.metadata.deb --> <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/opad-bin.install -->

- `opad-daemon`, `opad-gui` y `opadctl` en `/usr/bin`, con tosu y un `espflash` de versión fija en `/usr/lib/opad/`;
- la regla udev `/usr/lib/udev/rules.d/70-opad.rules`, para que puedas usar el teclado sin root y ModemManager no lo toque;
- la capacidad `cap_sys_ptrace` en `/usr/lib/opad/tosu/tosu`, para que tosu pueda leer osu! sin que tengas que bajar el `ptrace_scope` de todo el sistema (consulta [Solución de problemas §2](../troubleshooting.md#2-linux-tosu-no-puede-leer-osu-yama-ptrace_scope));
- una entrada de inicio automático al iniciar sesión que arranca el icono de bandeja de OPad, y una unidad de usuario de systemd `opad-daemon`.

No necesitas arrancar el daemon tú mismo: cuando `opad-gui` se inicia y no encuentra ningún daemon en marcha, arranca uno (mediante la unidad de usuario de systemd cuando está disponible). <!-- src: /desktop/gui/src/platform_linux.rs#start_daemon --> <!-- src: /desktop/gui/src/main.rs#App -->

Estos paquetes se actualizan solos desde la pestaña **Settings** → **🔄 Updates & Firmware** de la app. <!-- src: /desktop/crates/opad-update/src/origin.rs -->

### AppImage

```bash
chmod +x OPad-Linux-<version>.AppImage
./OPad-Linux-<version>.AppImage
```

La AppImage no instala nada a nivel de sistema, así que la primera vez que se ejecuta ofrece instalar la regla udev (te pide la contraseña mediante `pkexec`). También puedes instalar la regla tú mismo: <!-- src: /desktop/gui/src/platform_linux.rs#appimage_needing_udev_rule --> <!-- src: /packaging/linux/appimage/AppRun -->

```bash
./OPad-Linux-<version>.AppImage install-udev
```

El mismo archivo ejecuta también los demás programas: `./OPad-Linux-<version>.AppImage daemon` arranca el daemon y `./OPad-Linux-<version>.AppImage ctl <command>` ejecuta `opadctl`. <!-- src: /packaging/linux/appimage/AppRun -->

> [!NOTE]
> Una AppImage no puede dar a tosu la capacidad `cap_sys_ptrace`, así que en un sistema con el `ptrace_scope` de Yama por encima de 0 tienes que bajarlo para tener datos de osu! en vivo. Consulta [Solución de problemas §2](../troubleshooting.md#2-linux-tosu-no-puede-leer-osu-yama-ptrace_scope). <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Compilar desde el código fuente

Desde una copia del repositorio, el `PKGBUILD` de [`packaging/linux/arch/`](https://github.com/OPad-dev/OPad/blob/main/packaging/linux/arch/PKGBUILD) compila un paquete de Arch llamado `opad`, y `sudo make install` instala en `/usr/local` (regla udev incluida). [Compilar desde el código fuente](../develop/building.md) explica los detalles. <!-- src: /packaging/linux/arch/PKGBUILD --> <!-- src: /Makefile#install -->

Hay dos diferencias con los paquetes de las versiones publicadas:

- El `PKGBUILD` del código fuente ejecuta tosu con el `nodejs` del sistema, que no puede llevar `cap_sys_ptrace`; con `ptrace_scope` por encima de 0 tienes que bajarlo ([Solución de problemas §2](../troubleshooting.md#2-linux-tosu-no-puede-leer-osu-yama-ptrace_scope)). <!-- src: /packaging/linux/arch/PKGBUILD -->
- `make install-user` instala en `~/.local` y no instala la regla udev. Instálala una vez con: <!-- src: /Makefile#install-user -->

  ```bash
  sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
  sudo udevadm control --reload-rules && sudo udevadm trigger
  ```

### Comprueba tu configuración en Linux

```bash
opadctl setup
```

Indica si la regla udev está instalada y corresponde a esta versión, y si tosu puede leer la memoria de osu!, con el comando que lo arregla si no es así. <!-- src: /desktop/cli/src/main.rs#run_setup -->

## Windows

1. Descarga `OPad-Windows-Setup-<version>.exe` y ejecútalo.
2. Windows puede mostrar **Windows protected your PC**, porque el instalador todavía no está firmado. Haz clic en **More info** y luego en **Run anyway**. Si quieres, compara antes el archivo con la suma SHA-256 que aparece junto a él en la página de la versión. <!-- src: /README.md -->
3. Deja marcada **Start OPad when I log in** (la opción predeterminada) si quieres que el daemon y el icono de la bandeja se inicien con Windows.
4. Termina el instalador. Arranca OPad por ti.
<!-- src: /packaging/windows/installer.iss -->

El instalador no necesita permisos de administrador. Instala solo para tu usuario, en `%LOCALAPPDATA%\Programs\opad`, con `opad-gui.exe`, `opad-daemon.exe`, `opadctl.exe`, `espflash.exe` y `tosu\tosu.exe`. **Start OPad when I log in** añade `opad-daemon` y `opad-gui --tray` a `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Windows usa sus propios controladores `hidclass.sys` y `usbser.sys` para el teclado; no se instala nada más. <!-- src: /packaging/windows/installer.iss -->

## Comprueba que funciona

Abre **OPad** desde el menú de aplicaciones o el menú Inicio, o ejecuta:

```bash
opadctl status
```

Deberías ver `ESP32 Device:     Connected`, la versión de firmware del teclado y `tosu (osu!):      Active` en cuanto tosu se haya iniciado. En la app, la página **Device** muestra lo mismo. <!-- src: /desktop/cli/src/main.rs#Commands::Status --> <!-- src: /desktop/gui/src/pages.rs#device -->

Si no se detecta el teclado, consulta [Solución de problemas §3](../troubleshooting.md#3-linux-permisos-usb-y-el-puerto-serie).
