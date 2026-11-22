---
title: Desinstalación del software
description: Cómo desinstalar la suite de escritorio de OPad en Linux y Windows, qué datos se eliminan y qué permanece en el teclado.
sidebar:
  order: 8
---

Desinstalar la suite de escritorio elimina el daemon en segundo plano, la GUI, `opadctl`, el `tosu` incluido, las entradas de inicio automático y los datos de escritorio.

> [!NOTE]
> Desinstalar el software de escritorio **no tiene ningún efecto sobre el hardware del teclado**. OPad sigue funcionando como un teclado USB HID autónomo a 1000 Hz, y sus contadores de por vida, ajustes de teclas, registro de propiedad y calibración Hall Effect permanecen intactos en la flash NVS del teclado. Para borrar o desvincular el propio teclado, consulta [Recuperación, reflasheo y desvinculación](../recovery.md). <!-- src: /AGENTS.md --> <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

> [!WARNING]
> Tanto los desinstaladores de paquetes de Linux (`.deb`, `.rpm`, Arch) como el desinstalador de Windows eliminan el directorio de datos de escritorio de OPad (incluidos `opad.db`, las copias de seguridad automáticas, los ajustes, los diseños y `install.id`) sin preguntar. Exporta antes una copia con `opadctl export backup.json` o **Device** -> **Export backup** si deseas conservar una copia en disco. <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/windows/installer.iss -->

## Desinstalación en Linux

### Debian / Ubuntu (`.deb`)
```bash
sudo apt remove opad
# Or to also remove system conffiles:
sudo apt purge opad
```
Durante la desinstalación del paquete (nunca en una actualización), `prerm` ejecuta `/usr/lib/opad/uninstall-cleanup` (`packaging/linux/uninstall-cleanup.sh`) y `postrm` limpia los archivos del sistema: <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /packaging/linux/deb/postrm -->
- Detiene los procesos en ejecución `opad-gui`, `opad-daemon`, `opadctl` y `/usr/lib/opad/tosu/tosu`.
- Elimina `/usr/lib/opad`, `/etc/opad` y `/etc/xdg/autostart/opad-gui.desktop`, y recarga `udevadm`.
- Borra `$HOME/.local/share/opad`, `$HOME/.local/state/opad`, `~/.config/autostart/opad-gui.desktop`, el enlace simbólico habilitado `~/.config/systemd/user/default.target.wants/opad-daemon.service` y cualquier directorio `~/.config/tosu`, `~/.local/share/tosu` o `~/.cache/tosu` que contenga el marcador `.created-by-opad` de OPad (dejando intactos los directorios de `tosu` instalados por el usuario).

### Fedora / RHEL (`.rpm`)
```bash
sudo dnf remove opad
```
Ejecuta el mismo script `uninstall-cleanup` al desinstalar (no al actualizar). <!-- src: /packaging/linux/uninstall-cleanup.sh -->

### Arch Linux (`PKGBUILD`)
```bash
sudo pacman -R opad
```
(Usa `opad-bin` si lo instalaste desde un paquete binario de AUR.) <!-- src: /packaging/linux/arch/PKGBUILD --> <!-- src: /packaging/linux/uninstall-cleanup.sh -->

### AppImage, instalaciones desde código fuente o directorios XDG personalizados
Si usaste el AppImage, instalaste desde el código fuente (`make install-user`) o definiste rutas `$XDG_DATA_HOME` / `$XDG_STATE_HOME` personalizadas, elimínalos manualmente: <!-- src: /packaging/linux/uninstall-cleanup.sh --> <!-- src: /desktop/crates/opad-model/src/paths.rs -->
```bash
# Stop running processes
pkill -x opad-gui opad-daemon || true

# Remove AppImage or user-local binaries, and desktop data/logs
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/opad"
rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/opad"
rm -f ~/.config/autostart/opad-gui.desktop

# Optional: remove udev rule installed via AppImage or source
sudo rm -f /etc/udev/rules.d/70-opad.rules
sudo udevadm control --reload-rules && sudo udevadm trigger
```

## Desinstalación en Windows

1. Abre **Configuración de Windows** (**Windows Settings**) -> **Aplicaciones** (**Apps**) -> **Aplicaciones instaladas** (**Installed apps**) (o **Panel de control** -> **Programas y características**).
2. Selecciona **OPad** y haz clic en **Desinstalar** (**Uninstall**).
3. El desinstalador de Inno Setup realiza automáticamente: <!-- src: /packaging/windows/installer.iss -->
   - Detiene los procesos en ejecución `opad-daemon.exe`, `opad-gui.exe`, `opadctl.exe` y `tosu.exe` (`taskkill /F /T`).
   - Elimina el directorio de instalación (`%LOCALAPPDATA%\Programs\opad`).
   - Elimina las entradas de inicio automático `opad-daemon` y `opad-gui` en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
   - Elimina `%APPDATA%\opad` (que contiene `opad.db`, las copias automáticas, `daemon.log`, `tosu.log` y la carpeta temporal de actualizaciones).

(Las Named Pipes y los controladores COM/HID integrados de Windows no requieren limpieza manual.) <!-- src: /packaging/windows/installer.iss -->

## Cómo comprobarlo

- Confirma que no haya ningún proceso `opad-daemon` ni `opad-gui` en ejecución.
- Conecta tu OPad y escribe en un editor de texto: Key 1 y Key 2 siguen escribiendo con normalidad como un teclado USB HID autónomo.
