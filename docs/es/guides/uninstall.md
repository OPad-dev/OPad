---
title: Desinstalar el software
description: Elimina la app de escritorio de OPad en Linux o Windows, qué borra el desinstalador y por qué el teclado sigue funcionando después.
sidebar:
  order: 8
---

Desinstalar elimina el daemon, la app, `opadctl`, el tosu incluido, las entradas de inicio automático y los datos de OPad en este PC.

> [!NOTE]
> El teclado en sí no se ve afectado. Sigue funcionando como un teclado USB normal, y sus contadores de por vida, ajustes de teclas, calibración Hall Effect y registro del propietario se quedan en el teclado. Para borrar o ceder el propio teclado, consulta [Recuperación §7](../recovery.md#7-reflashear-y-desvincular-un-teclado). <!-- src: /AGENTS.md#1. Product rules --> <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

> [!WARNING]
> El desinstalador borra la carpeta de datos de OPad sin preguntar: la base de datos con tus ajustes, diseños y estadísticas de juego, las copias de seguridad automáticas y los registros. Si quieres conservar una copia, exporta antes una copia de seguridad (`opadctl export ~/opad-backup.json` o **Device** → **Export backup**) y guárdala fuera de esa carpeta. <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

## Linux

Usa el comando del paquete que instalaste:

```bash
sudo apt remove opad        # .deb
sudo dnf remove opad        # .rpm
sudo pacman -R opad-bin     # Arch release package (opad-bin)
sudo pacman -R opad         # Arch package built from the repository's PKGBUILD
```
<!-- src: /desktop/gui/Cargo.toml#package.metadata.deb --> <!-- src: /packaging/linux/arch/PKGBUILD-bin.in --> <!-- src: /packaging/linux/arch/PKGBUILD -->

En una desinstalación real (nunca en una actualización), el paquete ejecuta `/usr/lib/opad/uninstall-cleanup`, que: <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/linux/deb/prerm --> <!-- src: /packaging/linux/arch/opad-bin.install -->

- detiene `opad-gui`, `opad-daemon`, `opadctl` y el tosu incluido en todas las sesiones;
- para cada usuario, borra `~/.local/share/opad` y `~/.local/state/opad`, el archivo de inicio automático de la app `~/.config/autostart/opad-gui.desktop` y un enlace habilitado del servicio de usuario `opad-daemon`;
- borra `~/.config/tosu`, `~/.local/share/tosu` y `~/.cache/tosu` de tosu solo si los creó OPad, nunca un tosu que hayas instalado tú;
- borra la caché del actualizador `/var/cache/opad-update`.

La eliminación del paquete también borra `/usr/lib/opad` y `/etc/opad`, quita la regla udev y la entrada de inicio automático del sistema y recarga udev. <!-- src: /packaging/linux/deb/postrm --> <!-- src: /desktop/gui/Cargo.toml#package.metadata.generate-rpm -->

> [!NOTE]
> La limpieza solo conoce las carpetas predeterminadas. Si has definido tu propio `$XDG_DATA_HOME` o `$XDG_STATE_HOME`, borra tú mismo `$XDG_DATA_HOME/opad` y `$XDG_STATE_HOME/opad`. <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /desktop/crates/opad-model/src/paths.rs -->

### AppImage

La AppImage no tiene desinstalador. Elimínala a mano: <!-- src: /desktop/crates/opad-model/src/paths.rs --> <!-- src: /desktop/gui/src/platform_linux.rs --> <!-- src: /packaging/linux/appimage/AppRun -->

```bash
# Stop OPad
pkill -x opad-gui; pkill -x opad-daemon

# If you let the app install its systemd user service
systemctl --user disable --now opad-daemon.service
rm -f ~/.config/systemd/user/opad-daemon.service

# Data, logs and autostart
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/opad"
rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/opad"
rm -f ~/.config/autostart/opad-gui.desktop

# The udev rule the AppImage installed
sudo rm -f /etc/udev/rules.d/70-opad.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Después borra el archivo `.AppImage`.

### Instalado desde el código fuente

Desde la misma copia del repositorio, ejecuta `make uninstall-user` para una instalación con `make install-user`, o `sudo make uninstall` para `sudo make install`. Esto elimina los programas, los archivos de servicio y de inicio automático y `lib/opad`; borra las carpetas de datos y, en el caso de `install-user`, la regla udev que copiaste, como en los pasos de la AppImage de arriba. <!-- src: /Makefile#uninstall --> <!-- src: /Makefile#uninstall-user -->

## Windows

1. Abre **Settings** → **Apps** → **Installed apps** (o **Control Panel** → **Programs and Features**).
2. Busca **OPad** y haz clic en **Uninstall**.

El desinstalador: <!-- src: /packaging/windows/installer.iss -->

- detiene `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe` y `tosu.exe`;
- borra la carpeta de instalación `%LOCALAPPDATA%\Programs\opad`;
- quita las entradas de inicio de sesión `opad-daemon` y `opad-gui` de `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`;
- borra `%APPDATA%\opad`: la base de datos, las copias de seguridad automáticas, `daemon.log`, `tosu.log` y las descargas de actualizaciones.

No hay controladores que quitar: el teclado solo ha usado siempre los propios de Windows.

## Comprueba que ha funcionado

1. Comprueba que no hay ningún proceso `opad-daemon` u `opad-gui` en ejecución.
2. Conecta el teclado y escribe en un editor de texto: las teclas siguen funcionando, como un teclado USB independiente.
