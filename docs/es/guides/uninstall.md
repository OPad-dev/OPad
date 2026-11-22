---
title: Desinstalación del software
description: Procedimiento completo de desinstalación de la suite de escritorio de OPad en Linux y Windows, limpieza de registros de base de datos, servicios y reglas udev.
category: how-to
source_files:
  - /packaging/windows/installer.iss
  - /packaging/linux/uninstall-cleanup.sh
  - /packaging/linux/deb/postrm
  - /desktop/crates/opad-model/src/paths.rs
---

Si decides desinstalar la suite de escritorio de OPad, puedes eliminar todos los binarios de la aplicación, servicios en segundo plano, entradas de inicio automático y el estado de la base de datos local limpiamente de tu sistema. <!-- src: /packaging/linux/uninstall-cleanup.sh -->

> [!NOTE]
> Desinstalar la suite de escritorio tiene **cero impacto en el hardware del teclado**. El teclado continúa funcionando como un teclado USB HID independiente a 1000 Hz de inmediato (Regla de producto 2). Los contadores internos de vida útil del teclado, la calibración de los interruptores y los ajustes guardados en NVS permanecen intactos en el dispositivo. <!-- src: /packaging/linux/uninstall-cleanup.sh -->

## Desinstalación en Linux

### Debian / Ubuntu (`.deb`)
Para eliminar los paquetes de la aplicación:
```bash
sudo apt remove opad
```
Para purgar por completo los archivos de configuración y las reglas udev:
```bash
sudo apt purge opad
```
Los scripts de empaquetado ejecutan `packaging/linux/uninstall-cleanup.sh` y `postrm`: <!-- src: /packaging/linux/deb/postrm -->
- Detienen las instancias en ejecución de `opad-daemon`, `opad-gui` y el binario integrado `tosu`.
- Eliminan `/usr/lib/opad` y `/etc/xdg/autostart/opad-gui.desktop`. <!-- src: /packaging/linux/deb/postrm -->
- Recargan las reglas udev para liberar los permisos del dispositivo.

### Fedora / RHEL (`.rpm`)
```bash
sudo dnf remove opad
```

### Arch Linux (AUR)
```bash
sudo pacman -R opad-bin
```

### Limpieza manual de datos en Linux
Si deseas purgar todas las bases de datos locales, los archivos de identidad de instalación y los registros creados por el demonio: <!-- src: /desktop/crates/opad-model/src/paths.rs -->
```bash
# Eliminar la base de datos persistente y las copias de seguridad
rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/opad"

# Eliminar los registros
rm -rf "${XDG_STATE_HOME:-$HOME/.local/state}/opad"
```

## Desinstalación en Windows

En Windows 10 y 11, OPad proporciona un desinstalador limpio de Inno Setup: <!-- src: /packaging/windows/installer.iss -->

1. Abre **Configuración de Windows > Aplicaciones > Aplicaciones instaladas** (o **Panel de control > Programas y características**).
2. Localiza **OPad** en la lista y haz clic en **Desinstalar**.
3. **Aviso de retención de datos**:
   - El desinstalador preguntará si deseas **"¿Eliminar todos los ajustes de usuario, el historial de la base de datos y los registros?"**.
   - Si haces clic en **Sí**, el desinstalador eliminará `%APPDATA%\opad` (incluyendo `opad.db` y el UUID de instalación). Una reinstalación posterior generará una nueva identidad de instalación. <!-- src: /desktop/crates/opad-model/src/paths.rs#data_dir -->
   - Si haces clic en **No**, tus contadores locales de pulsaciones y las copias de seguridad de los diseños de pantalla se conservarán en `%APPDATA%\opad` para futuras reinstalaciones.
4. El desinstalador elimina los binarios de `C:\Program Files\OPad`, detiene los procesos en segundo plano del demonio que estén en ejecución y elimina la entrada de inicio de sesión del usuario en el registro. <!-- src: /packaging/windows/installer.iss -->
