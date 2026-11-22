---
title: Instalación de la suite de escritorio
description: Cómo instalar la suite de escritorio de OPad (opad-daemon, opad-gui, opadctl y tosu incluido) en Linux y Windows.
---

La suite de escritorio opcional de OPad añade telemetría en vivo de osu!, un diseñador de diseño de pantalla, sincronización de contadores de por vida, diagnósticos interactivos, actualizaciones de firmware y ajuste Hall Effect. Se compone de: <!-- src: /README.md --> <!-- src: /desktop/crates/opad-model/src/paths.rs -->

- `opad-daemon`: servicio en segundo plano que gestiona la comunicación USB CDC, la persistencia en SQLite y la supervisión de `tosu`.
- `opad-gui`: aplicación gráfica de configuración, diseñador de pantalla, diagnósticos y bandeja del sistema (construida con `iced`).
- `opadctl`: herramienta de línea de comandos para administración y flasheo de firmware.
- `tosu`: lector de memoria incluido que suministra telemetría de osu! en vivo (LGPL-3.0; puedes sustituirlo por el tuyo mediante `OPAD_TOSU_PATH` o **Settings** -> **ℹ About & tosu**). <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

## Instalación en Linux

Los paquetes precompilados para Linux se publican en la página de [GitHub Releases](https://github.com/OPad-dev/OPad/releases) (dirigidos a glibc 2.28+, cubriendo Ubuntu 22.04/24.04/26.04, Debian 12/13, RHEL/AlmaLinux/Rocky 9–10, Fedora y Arch). <!-- src: /README.md -->

### Debian / Ubuntu / Pop!_OS (`.deb`)
```bash
sudo apt install ./opad_<version>_amd64.deb
```
O con `dpkg`:
```bash
sudo dpkg -i opad_<version>_amd64.deb
sudo apt-get install -f
```
El paquete instala: <!-- src: /packaging/linux/deb/postinst --> <!-- src: /Makefile -->
- Binarios en `/usr/bin/`, `tosu` incluido en `/usr/lib/opad/tosu/` y `espflash` fijado en `/usr/lib/opad/bin/`.
- Reglas `udev` en `/etc/udev/rules.d/70-opad.rules` (y recarga `udevadm`).
- `cap_sys_ptrace=eip` en `/usr/lib/opad/tosu/tosu` para que `tosu` pueda leer la memoria de `osu!` con Yama `ptrace_scope` 1–2.
- La unidad de usuario de systemd `opad-daemon.service` y la entrada de inicio automático en escritorio/bandeja del sistema.

### Fedora / RHEL / openSUSE (`.rpm`)
```bash
sudo dnf install ./opad-<version>.x86_64.rpm
```
Configura los mismos binarios, la regla `udev`, la capacidad `cap_sys_ptrace=eip` y la unidad de usuario de systemd que el paquete `.deb`. <!-- src: /README.md -->

### Arch Linux (`PKGBUILD`)
Compila e instala desde `packaging/linux/arch/PKGBUILD` del repositorio (nombre del paquete `opad`): <!-- src: /packaging/linux/arch/PKGBUILD -->
```bash
makepkg -si
```
En Arch, `tosu` se ejecuta bajo el `nodejs` del sistema (`TOSU_STANDALONE=0`), por lo que no se aplican capacidades de archivo a `node`; consulta [Solución de problemas §2](../troubleshooting.md) si `ptrace_scope > 0`. <!-- src: /packaging/linux/arch/PKGBUILD -->

### AppImage autónomo
```bash
chmod +x opad-x86_64.AppImage
./opad-x86_64.AppImage
```
En la primera ejecución, el AppImage ofrece instalar la regla `udev`, o puedes instalarla explícitamente con `pkexec`: <!-- src: /packaging/linux/appimage/AppRun -->
```bash
./opad-x86_64.AppImage install-udev
```
También puedes invocar el daemon o la CLI incluidos a través del AppImage: `./opad-x86_64.AppImage daemon` o `./opad-x86_64.AppImage ctl <command>`. Como un AppImage se monta con `nosuid`, `cap_sys_ptrace` no se aplica a su `tosu` incluido; consulta [Solución de problemas §2](../troubleshooting.md). <!-- src: /packaging/linux/appimage/AppRun --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Regla `udev` manual (instalaciones desde código fuente)
Si instalaste desde el código fuente con `make && sudo make install` o `make install-user`, instala `70-opad.rules` para que usuarios sin root puedan abrir el puerto CDC, el nodo USB y el nodo de entrada, y para que ModemManager no sondee el teclado: <!-- src: /packaging/linux/udev/70-opad.rules -->
```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```
Ejecuta `opadctl setup` para comprobar las reglas `udev` y la configuración de `ptrace`. <!-- src: /desktop/cli/src/main.rs#run_setup -->

## Instalación en Windows

Descarga `opad-setup.exe` (u `opad-setup-<version>.exe`) desde GitHub Releases y ejecútalo. <!-- src: /packaging/windows/installer.iss -->

1. **SmartScreen**: Hasta que se habilite la firma de código, Windows muestra "Windows protected your PC". Haz clic en **More info** -> **Run anyway** (puedes verificar antes el archivo contra `SHA256SUMS` en la página de lanzamiento). <!-- src: /README.md -->
2. **Instalación por usuario**: Se instala sin permisos de administrador (`PrivilegesRequired=lowest`) en `%LOCALAPPDATA%\Programs\opad` (`DefaultDirName={localappdata}\Programs\opad`). <!-- src: /packaging/windows/installer.iss -->
3. **Archivos instalados**: `opad-gui.exe`, `opad-daemon.exe`, `opadctl.exe`, `espflash.exe`, `install-origin` y `tosu\tosu.exe`. <!-- src: /packaging/windows/installer.iss -->
4. **Inicio automático**: La tarea opcional **Start OPad when I log in** (marcada por defecto) añade `opad-daemon` y `opad-gui` (`--tray`) en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. <!-- src: /packaging/windows/installer.iss -->
5. **Sin controladores de terceros**: Windows 10/11 usa los controladores integrados `hidclass.sys` y `usbser.sys`. <!-- src: /README.md -->

## Iniciar el software

Abre **OPad** desde el menú de aplicaciones o el Menú Inicio:
- `opad-gui` se conecta a `opad-daemon` mediante IPC local (`$XDG_RUNTIME_DIR/opad/daemon.sock` en Linux, `\\.\pipe\opad-ipc-<SID>` en Windows). <!-- src: /desktop/crates/opad-model/src/paths.rs --> <!-- src: /desktop/crates/opad-ipc/src/transport/windows.rs#get_socket_path -->
- Verifica que todo esté conectado con `opadctl status` o en **Dashboard** de `opad-gui`. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->
