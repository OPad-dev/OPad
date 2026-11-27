---
title: Solución de problemas
description: Soluciones para caracteres incorrectos en distribuciones no QWERTY, falta de datos de tosu en Linux, permisos USB, avisos de Windows SmartScreen y del antivirus, y un teclado que deja de responder.
sidebar:
  order: 3
---

Cada sección empieza por el síntoma. Los números de sección son fijos, porque `opadctl` y la app remiten a ellos.

## 1. Distribuciones de teclado (QWERTY, AZERTY, QWERTZ)

**Síntoma:** las teclas escriben letras equivocadas, por ejemplo `w` en lugar de `z`.

Un teclado USB envía *posiciones* de tecla (IDs de uso HID), no letras, y la distribución de teclado de tu sistema convierte cada posición en un carácter. Los valores predeterminados de OPad son las posiciones de `z`, `x`, `c` y `` ` `` en un teclado estadounidense: <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/main/config/device_config.h -->

| Control | Uso HID | QWERTY US / UK | AZERTY francés | QWERTZ alemán |
|---|---|---|---|---|
| Key 1 | `0x1D` | `z` | `w` | `y` |
| Key 2 | `0x1B` | `x` | `x` | `x` |
| Key 3 (módulos V3) | `0x06` | `c` | `c` | `c` |
| Pantalla táctil (Quick Retry) | `0x35` | `` ` `` | `²` | `^` |

No hay nada roto. Arréglalo en cualquiera de estos dos sitios:

- **En osu!** (lo más rápido): abre los ajustes de asignación de teclas y pulsa cada tecla de OPad para asignar el carácter que dé tu distribución.
- **En la app de OPad:** abre **Keys**, haz clic en una tecla en **Bindings** y pulsa la letra que debe enviar (Key 1, Key 2, y Key 3 en un módulo V3), y después haz clic en **Save to pad**. El teclado las guarda, así que le acompañan a cualquier PC. <!-- src: /desktop/gui/src/keys.rs#bindings --> <!-- src: /firmware/main/config/device_config.c -->

## 2. Linux: tosu no puede leer osu! (Yama ptrace_scope)

**Síntoma:** tosu se ejecuta y `opadctl status` muestra `tosu (osu!): Active`, pero el teclado no muestra datos del mapa ni de la partida.

tosu lee la memoria de osu!. En Linux, el módulo de seguridad Yama (`/proc/sys/kernel/yama/ptrace_scope`) lo bloquea cuando el valor es mayor que 0, porque osu! no es un proceso hijo de tosu. Un binario de tosu con la capacidad `cap_sys_ptrace` queda exento en los niveles 1 y 2. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

Primero, averigua en qué caso estás:

```bash
opadctl doctor
```

Muestra tu `ptrace_scope`, si tosu tiene la capacidad y el comando exacto que debes ejecutar. **Troubleshooting** → **Connection** en `opad-gui` dice lo mismo. <!-- src: /desktop/cli/src/main.rs#run_doctor, /desktop/gui/src/troubleshooting.rs#view_connection -->

### Solución A: dar la capacidad a tosu (paquetes e instalaciones en `~/.local`)

Los paquetes `.deb`, `.rpm` y de Arch de las versiones publicadas ya lo hacen al instalarse. Si se ha perdido, o en una instalación con `make install-user`, ejecuta la línea que corresponda a tu instalación: <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/opad-bin.install --> <!-- src: /Makefile#install-user -->

```bash
# .deb, .rpm or Arch release package
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu

# make install-user
sudo setcap cap_sys_ptrace=eip ~/.local/lib/opad/tosu/tosu
```

Así la protección de todo el sistema sigue activa y solo tosu queda exento.

### Solución B: bajar ptrace_scope (AppImage y paquete de Arch compilado desde el código fuente)

La capacidad no puede funcionar en dos casos, y `opadctl doctor` te lo indica: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access --> <!-- src: /packaging/linux/arch/PKGBUILD -->

- la **AppImage**, porque sus archivos se montan con `nosuid`, que ignora las capacidades de archivo;
- el paquete de Arch compilado con el **`PKGBUILD` del código fuente** del repositorio, que ejecuta tosu con el `node` del sistema; dar la capacidad a `node` se la daría a todos los programas de Node.

En esos casos, baja el nivel a 0. Hasta el próximo reinicio:

```bash
echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
```

Para conservarlo tras los reinicios, pon esta línea en `/etc/sysctl.d/10-ptrace.conf`:

```ini
kernel.yama.ptrace_scope = 0
```

y aplícala con `sudo sysctl --system`.

> [!NOTE]
> Con `ptrace_scope` a 3 nada puede leer la memoria de otro proceso, con o sin la capacidad, y el valor no se puede bajar hasta que reinicies. Ponlo a 0 en `/etc/sysctl.d/` y reinicia. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### tosu se cierra al instante con `GLIBC_2.xx not found`

El tosu que incluye OPad está recompilado para funcionar con glibc 2.28 o posterior, así que arranca en todas las distribuciones compatibles. La propia compilación para Linux del tosu original necesita glibc 2.38, que Ubuntu 22.04, Debian 12 y RHEL 9 no tienen. Solo ves este error con un tosu que hayas aportado tú (`OPAD_TOSU_PATH`) o con uno compilado con `make tosu TOSU_PORTABLE=0`. Usa el incluido en su lugar. <!-- src: /Makefile#TOSU_PORTABLE -->

## 3. Linux: permisos USB y el puerto serie

### El daemon no encuentra el teclado o informa de `Permission denied`

Tu usuario necesita la regla udev `70-opad.rules` para abrir el puerto serie del teclado (`/dev/ttyACM*`), su dispositivo USB y su dispositivo de entrada (que usa el probador de interruptores de la app). La regla también impide que ModemManager sondee el teclado. <!-- src: /packaging/linux/udev/70-opad.rules --> <!-- src: /desktop/gui/src/troubleshooting.rs#open_opad_evdev -->

1. Compruébala:

   ```bash
   opadctl doctor
   ```

   Indica si la regla está instalada (en `/etc/udev/rules.d/` o `/usr/lib/udev/rules.d/`) y si corresponde a esta versión. <!-- src: /desktop/cli/src/main.rs#run_doctor -->
2. Si falta:
   - **Paquete `.deb`, `.rpm` o de Arch:** reinstala el paquete; incluye la regla en `/usr/lib/udev/rules.d/`. <!-- src: /desktop/gui/Cargo.toml#package.metadata.deb -->
   - **AppImage:** acepta el aviso cuando se inicie la app o ejecuta `./OPad-Linux-<version>.AppImage install-udev`. <!-- src: /packaging/linux/appimage/AppRun -->
   - **Copia del código fuente:**

     ```bash
     sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
     sudo udevadm control --reload-rules && sudo udevadm trigger
     ```
3. Desconecta el teclado y vuelve a conectarlo.

Mantén el nombre del archivo empezando por `70-`. La regla concede el acceso con la etiqueta `uaccess` de systemd, que solo funciona en una regla que se ordene antes de `73-seat-late.rules`. <!-- src: /packaging/linux/udev/70-opad.rules -->

La regla cubre el ID USB de la app `303a:8415`, el `303a:1001` del bootloader del ESP32-S3 y el `303a:4001` que usaba el firmware release candidate. <!-- src: /packaging/linux/udev/70-opad.rules -->

### El puerto serie está ocupado

Mientras `opad-daemon` se ejecuta, retiene el puerto serie del teclado (`/dev/ttyACM0` en Linux, un puerto `COM` en Windows). `opadctl flash`, `opadctl pad bootloader` y la actualización de firmware de la app piden por ti al daemon que lo suelte. Para cualquier otra herramienta serie, detén antes el daemon. <!-- src: /desktop/cli/src/main.rs#prepare_flash -->

### Una tecla no hace nada tras cambiar un pin

Compara la etiqueta GPIO impresa en la placa junto al cable con **Key Pins** en `opadctl status`. La lista de pines de la app nombra posiciones del cabezal según el esquemático, como `GPIO14 (P2 pin 11)`, y es fácil contarlas mal en el cabezal. Consulta [Cambiar los pines de las teclas](guides/change-key-pins.md). <!-- src: /AGENTS.md#6. Hardware and debugging facts --> <!-- src: /desktop/crates/opad-model/src/lib.rs#KEY_PINS -->

## 4. Windows: SmartScreen y antivirus

### "Windows protected your PC"

El instalador todavía no está firmado, así que SmartScreen avisa sobre él. <!-- src: /README.md -->

1. Haz clic en **More info**.
2. Haz clic en **Run anyway**.

Si quieres asegurarte de que el archivo es el auténtico, compáralo antes con la suma SHA-256 que aparece junto a él en la página de la versión en GitHub.

Nunca necesitas instalar un controlador: Windows usa sus propios `hidclass.sys` y `usbser.sys`, y el puerto serie del teclado aparece en Device Manager como **OPad (COMx)**. <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_ms_os_20_desc -->

### Los datos de osu! en vivo se detienen: el antivirus y `tosu.exe`

tosu lee la memoria de osu!, y algunos antivirus lo marcan o lo ponen en cuarentena por ello. Si el teclado deja de mostrar datos del mapa:

1. Revisa la cuarentena de tu antivirus (en Seguridad de Windows: **Protection history**).
2. Restaura `tosu.exe` y añade una exclusión para `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`. <!-- src: /packaging/windows/installer.iss -->

## 5. Recuperación del firmware

**Síntoma:** el teclado ha dejado de responder o ya no aparece como OPad (`303a:8415`).

Nunca necesitas los botones BOOT o RESET de la placa.

1. Ejecuta el comando de recuperación con una carpeta que contenga el firmware: una carpeta de compilación de ESP-IDF o una carpeta con los archivos `.bin` de una versión publicada del firmware: <!-- src: /desktop/cli/src/main.rs#run -->

   ```bash
   opadctl recover firmware/build
   ```

2. Cuando te lo pida, desconecta el cable USB del teclado y vuelve a conectarlo. `opadctl` atrapa el bootloader integrado del ESP32-S3 en el momento en que aparece al encenderse (`303a:1001`) y escribe el firmware completo por USB. <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->

[Recuperación §7](recovery.md#7-reflashear-y-desvincular-un-teclado) tiene los detalles, además de qué ocurre cuando se interrumpe una actualización y cómo reflashear o desvincular un teclado.
