---
title: Instalar la suite de escritorio
description: Guía de instalación para la suite de aplicaciones de escritorio de OPad (daemon, GUI, opadctl, tosu) en Linux (.deb, .rpm, Arch AUR, AppImage) y Windows.
---

La suite de software de escritorio de OPad proporciona telemetría HUD en tiempo real durante la partida, diseño visual de pantallas, diagnósticos interactivos de hardware y calibración de interruptores magnéticos Hall Effect. La suite consta de cuatro componentes principales: <!-- src: /desktop/crates/opad-model/src/paths.rs -->
- `opad-daemon`: Servicio en segundo plano que gestiona la comunicación serie, la persistencia de contadores y la supervisión de tosu.
- `opad-gui`: Interfaz gráfica construida con iced.
- `opadctl`: Utilidad de gestión y flasheo por línea de comandos.
- `tosu`: Lector de memoria integrado que transmite estadísticas de beatmaps en vivo desde osu!.

## Instalación en Linux

OPad se distribuye en múltiples formatos de paquetes nativos para Linux. <!-- src: /packaging/linux/deb/postinst -->

### Ubuntu / Debian (`.deb`)
Descarga el paquete `.deb` más reciente desde la página de [Releases](https://github.com/OPad-dev/OPad/releases):
```bash
sudo dpkg -i opad_<version>_amd64.deb
sudo apt-get install -f
```
El paquete instala automáticamente:
- Binarios en `/usr/bin/` y tosu integrado en `/usr/lib/opad/tosu/`. <!-- src: /desktop/crates/opad-model/src/paths.rs#install_lib_dir -->
- Reglas udev para el hardware (`/etc/udev/rules.d/70-opad.rules`). <!-- src: /packaging/linux/udev/70-opad.rules -->
- Configura `cap_sys_ptrace=eip` en `/usr/lib/opad/tosu/tosu` para que tosu pueda leer la memoria del proceso de osu! bajo restricciones de Yama ptrace sin requerir permisos de root. <!-- src: /packaging/linux/deb/postinst -->
- Instala los archivos de servicio de usuario de systemd (`opad-daemon.service`).

### Fedora / RHEL (`.rpm`)
Descarga el paquete `.rpm` e instálalo con dnf:
```bash
sudo dnf install ./opad-<version>.x86_64.rpm
```

### Arch Linux (AUR)
Instálalo mediante tu asistente de AUR preferido:
```bash
yay -S opad-bin
```

### AppImage independiente
Descarga el archivo `.AppImage` y dale permisos de ejecución:
```bash
chmod +x OPad-<version>-x86_64.AppImage
```
Debido a que las AppImages se ejecutan en el espacio de usuario, instala la regla udev requerida ejecutando: <!-- src: /packaging/linux/appimage/AppRun -->
```bash
./OPad-<version>-x86_64.AppImage install-udev
```
A continuación, ejecuta la AppImage directamente para iniciar `opad-gui`.

### Reglas udev de Linux (`70-opad.rules`)
Para permitir que los usuarios estándar con sesión iniciada se comuniquen con el puerto serie CDC del teclado sin necesidad de `sudo`, se instala la siguiente regla: <!-- src: /packaging/linux/udev/70-opad.rules -->
```udev
ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", ENV{ID_MM_DEVICE_IGNORE}="1"
SUBSYSTEM=="tty", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", TAG+="uaccess"
SUBSYSTEM=="input", ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", TAG+="uaccess"
```
*Nota: Se instruye explícitamente a ModemManager para que ignore el teclado (`ENV{ID_MM_DEVICE_IGNORE}="1"`), evitando que el tráfico de sondeo de módems AT corrompa la conexión serie.* <!-- src: /packaging/linux/udev/70-opad.rules -->

## Instalación en Windows

Descarga el instalador oficial para Windows (`OPad-Setup-<version>.exe`) desde GitHub Releases. <!-- src: /packaging/windows/installer.iss -->

1. **Ejecutar el instalador**: Ejecuta `OPad-Setup-<version>.exe`.
2. **Directorio de instalación**: Se instala de manera predeterminada en `C:\Program Files\OPad`. <!-- src: /packaging/windows/installer.iss -->
3. **Componentes**: Instala `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe` y el ejecutable integrado `tosu.exe`.
4. **Inicio automático**: Ofrece iniciar `opad-daemon` de forma silenciosa al iniciar sesión en Windows a través de la clave de Registro Run (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
5. **Sin controladores de terceros**: Windows 10 y 11 utilizan controladores integrados USB HID y USB Serial (CDC ACM) de forma nativa.

## Iniciar el software

Una vez instalado:
- Inicia **OPad** desde tu menú de aplicaciones o menú de inicio.
- `opad-gui` se conectará a `opad-daemon` a través de IPC local (`daemon.sock` en Linux, `\\.\pipe\opad-ipc-<sid>` en Windows). <!-- src: /desktop/crates/opad-model/src/paths.rs -->
- Si el demonio no está en ejecución, `opad-gui` lo iniciará automáticamente en segundo plano.
