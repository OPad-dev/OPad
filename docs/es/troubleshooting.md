---
title: Guía de solución de problemas
description: Problemas frecuentes con OPad, distribuciones de teclado, Yama ptrace en Linux para tosu, permisos USB, Windows SmartScreen y recuperación de firmware.
sidebar:
  order: 3
---

Esta guía cubre los problemas más comunes con las asignaciones de teclas de OPad, la telemetría de `tosu`, los permisos serie USB, los avisos de Windows SmartScreen/antivirus y la recuperación de firmware.

---

## 1. Distribuciones de teclado y códigos de escaneo (QWERTY, AZERTY, QWERTZ)

### Cómo funcionan los códigos de escaneo en OPad
OPad es un teclado compuesto USB HID a 1000 Hz. Los teclados USB HID envían **USB HID Keyboard Usage IDs** (códigos de escaneo de hardware), no letras localizadas; la distribución de teclado de tu sistema operativo traduce cada código a un carácter. <!-- src: /firmware/main/usb/usb_descriptors.c --> <!-- src: /firmware/main/usb/usb_hid.c -->

Códigos de escaneo predeterminados del firmware: <!-- src: /firmware/main/usb/usb_hid.c -->
- **Key 1**: `0x1D` (`Z` en US QWERTY)
- **Key 2**: `0x1B` (`X` en US QWERTY)
- **Key 3** (módulos V3 de 3 teclas): `0x06` (`C` en US QWERTY)
- **Pantalla táctil Quick Retry**: `0x35` (`` ` `` / `~` en US QWERTY)

### Comportamiento en distribuciones no QWERTY

| Distribución del sistema operativo | La Key 1 física envía | El SO la interpreta como | Acción recomendada |
|---|---|---|---|
| **US / UK QWERTY** | Usage `0x1D` | **`Z`** | Ninguna necesaria (`Z` / `X` por defecto en osu!) |
| **AZERTY francés** | Usage `0x1D` | **`W`** | En osu!, asigna la tecla izquierda a `W` o reasigna Key 1 en OPad |
| **QWERTZ alemán** | Usage `0x1D` | **`Y`** | En osu!, asigna la tecla izquierda a `Y` o reasigna Key 1 en OPad |
| **Dvorak / Colemak** | Usage `0x1D` | Depende de la distribución | Reasigna en osu! o en OPad |

### Cómo ajustar las asignaciones de teclas
1. **En osu! / osu!lazer (más rápido)**: Abre **Options -> Key Bindings** en osu! y pulsa Key 1 y Key 2 en tu OPad para asignar el carácter que genere tu distribución.
2. **En `opad-gui`**: Abre **Settings** -> **⌨ Keypad**, edita **KEY 1** / **KEY 2** (y **KEY 3** en teclados V3 de 3 teclas) y haz clic en **Save settings**. Los códigos se guardan en la NVS del teclado para persistir entre distintos PCs. <!-- src: /desktop/gui/src/pages.rs#settings --> <!-- src: /firmware/main/config/device_config.c -->

---

## 2. Ámbito ptrace de Yama en Linux y telemetría de memoria de tosu

### El problema
OPad muestra en su pantalla precisión, combo, unstable rate (UR) y telemetría del mapa en vivo mediante `tosu` (`ws://127.0.0.1:24050/websocket/v2`). En Linux, `tosu` lee la memoria del proceso `osu!` en ejecución (Wine/Proton o `osu!lazer` nativo). <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT -->

Las distribuciones de Linux con el **módulo de seguridad Yama** controlan la inspección de memoria de procesos a través de `/proc/sys/kernel/yama/ptrace_scope`: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->
- **Ámbito 0**: Los procesos con el mismo UID pueden inspeccionarse entre sí.
- **Ámbito 1 (predeterminado en Ubuntu/Debian)**: Los procesos solo pueden usar `ptrace` sobre hijos directos (`osu!` no es hijo de `tosu`).
- **Ámbito 2**: Solo los procesos con `CAP_SYS_PTRACE` pueden inspeccionar memoria.
- **Ámbito 3**: No se permite `ptrace` hasta reiniciar (no se puede bajar en ejecución; pon `0` en `/etc/sysctl.d/` y reinicia).

Cuando `ptrace_scope` es `1` o superior y `tosu` carece de `cap_sys_ptrace`, `tosu` no puede leer la memoria de `osu!` y la telemetría en vivo queda vacía. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Solución 1: Capacidad de archivo (recomendada para `.deb` / `.rpm` / instalaciones ELF locales)
Concede `cap_sys_ptrace=eip` únicamente al binario de `tosu` para mantener intacta la protección `ptrace` del resto del sistema: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access --> <!-- src: /packaging/linux/deb/postinst -->

```bash
# If installed via .deb or .rpm package:
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu

# If installed locally (~/.local):
sudo setcap cap_sys_ptrace=eip ~/.local/lib/opad/tosu/tosu
```

> [!NOTE]
> Los paquetes `.deb` y `.rpm` aplican `setcap cap_sys_ptrace=eip` automáticamente al instalarse. El **AppImage** no puede usar capacidades de archivo porque su montaje FUSE/squashfs es `nosuid`, y el **paquete de Arch** ejecuta `tosu` con el ejecutable `node` del sistema (dar la capacidad a `node` se la concedería a cualquier programa de Node). En esos dos casos, usa la Solución 2. Tanto `opadctl setup` como la página Diagnostics de `opad-gui` informan qué caso aplica a tu instalación. <!-- src: /packaging/linux/deb/postinst --> <!-- src: /packaging/linux/arch/PKGBUILD --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

### Solución 2: Ámbito ptrace global del sistema
Permite `ptrace` directamente entre procesos del mismo usuario: <!-- src: /desktop/crates/opad-tosu/src/lib.rs#ptrace_access -->

**Temporal (hasta reiniciar):**
```bash
echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
```

**Persistente (entre reinicios):**
Crea o edita `/etc/sysctl.d/10-ptrace.conf`:
```ini
kernel.yama.ptrace_scope = 0
```
Luego aplica los cambios:
```bash
sudo sysctl --system
```

### `tosu` se cierra de inmediato con `GLIBC_2.xx not found`
El `@tosuapp/lazer-calculator` precompilado del proyecto original de `tosu` requiere glibc 2.38, lo cual falla en distribuciones más antiguas como Ubuntu 22.04, Debian 12 y RHEL 9. Las compilaciones de lanzamiento de OPad lo recompilan contra glibc 2.28 (`TOSU_PORTABLE=1` por defecto en `Makefile`), de modo que el `tosu` incluido funciona en todas las distribuciones soportadas. Solo verás este error con un `tosu` personalizado (`OPAD_TOSU_PATH`) o uno compilado con `make tosu TOSU_PORTABLE=0`. <!-- src: /Makefile --> <!-- src: /desktop/crates/opad-tosu/src/lib.rs#find_tosu_binary -->

---

## 3. Permisos USB y acceso al puerto serie (Linux)

### Síntoma: `Permission denied (os error 13)` o el daemon no detecta el teclado
En Linux, los usuarios estándar necesitan una regla `udev` para acceder al dispositivo serie CDC del teclado (`/dev/ttyACM*`), al nodo de dispositivo USB y al nodo de entrada (necesario para el probador de rebote de interruptores de `opad-gui`). <!-- src: /packaging/linux/udev/70-opad.rules --> <!-- src: /desktop/gui/src/diagnostics.rs#open_opad_evdev -->

### Solución
Los paquetes `.deb` y `.rpm` instalan `/etc/udev/rules.d/70-opad.rules` automáticamente. En el AppImage, acepta el aviso de la primera ejecución o ejecuta `./OPad-Linux-<version>.AppImage install-udev`. Desde el código fuente: <!-- src: /packaging/linux/appimage/AppRun --> <!-- src: /packaging/linux/udev/70-opad.rules -->

```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Mantén el prefijo `70-` en el nombre del archivo para que se ordene antes de `73-seat-late.rules`, donde `systemd` aplica las ACL `TAG+="uaccess"` a la sesión de usuario activa. Ejecuta `opadctl setup` para verificar que la regla instalada coincide con la integrada. <!-- src: /packaging/linux/udev/70-opad.rules --> <!-- src: /desktop/cli/src/main.rs#run_setup -->

### Interferencia de ModemManager
ModemManager sondea los puertos CDC ACM recién conectados con comandos `AT`, lo cual puede interferir con el entramado serie de OPad. `70-opad.rules` desactiva el sondeo tanto para el PID de la aplicación (`303a:4001`) como para el del bootloader ROM (`303a:1001`): <!-- src: /packaging/linux/udev/70-opad.rules -->

```udev
ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", ENV{ID_MM_DEVICE_IGNORE}="1"
```

### Puerto serie ocupado
Mientras `opad-daemon` se ejecuta, mantiene abierto el puerto CDC del teclado (`/dev/ttyACM0` en Linux, `COMx` en Windows). Usa `opadctl flash` (que pide al daemon que libere el puerto automáticamente) o detén `opad-daemon` antes de abrir el puerto con otra herramienta. <!-- src: /desktop/cli/src/main.rs#Commands::Flash -->

### Las teclas no hacen nada tras cambiar pines
Compara la etiqueta física de serigrafía del GPIO al que está conectado el cable del interruptor con **Key Pins** en `opadctl status`. El selector de pines de la GUI muestra posiciones del cabezal del esquemático (por ejemplo, `GPIO14 (P2 pin 11)`), que son fáciles de confundir al contar pines en la placa. <!-- src: /AGENTS.md --> <!-- src: /desktop/gui/src/pages.rs#settings -->

---

## 4. Windows SmartScreen y permisos

### Windows Defender SmartScreen
El instalador de Windows (`OPad-Windows-Setup-<version>.exe`) se compila en GitHub Actions y no está firmado hasta que se habilite la firma de código. Si SmartScreen muestra "Windows protected your PC": <!-- src: /packaging/windows/installer.iss --> <!-- src: /README.md -->
1. Haz clic en **More info**.
2. Haz clic en **Run anyway**.
3. Opcionalmente comprueba la descarga con el SHA-256 que la página del lanzamiento de GitHub muestra junto a ella.

Windows 10/11 usa controladores integrados USB HID y CDC (`usbser.sys`) sin necesidad de instalar controladores; el descriptor MS OS 2.0 de OPad etiqueta el puerto serie como **OPad (COMx)** en el Administrador de dispositivos. <!-- src: /firmware/main/usb/usb_descriptors.c -->

### Falsos positivos de antivirus en `tosu.exe`
Dado que `tosu.exe` lee la memoria virtual de `osu!.exe`, algunos antivirus lo marcan o lo ponen en cuarentena. Si la telemetría deja de actualizarse en Windows: <!-- src: /packaging/windows/installer.iss -->
1. Comprueba el Historial de protección de Seguridad de Windows / cuarentena del antivirus.
2. Restaura y excluye `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`. <!-- src: /packaging/windows/installer.iss -->

---

## 5. Recuperación de firmware y reflasheo

Si el teclado deja de responder o deja de enumerar como OPad (`303a:4001`): <!-- src: /desktop/cli/src/main.rs#Commands::Recover --> <!-- src: /desktop/crates/opad-device/src/flash.rs#recover -->
1. Ejecuta el comando de recuperación sin botones indicando un directorio de compilación de ESP-IDF o un lanzamiento de firmware desempaquetado:
   ```bash
   opadctl recover firmware/build
   ```
2. Desconecta y vuelve a conectar el cable USB cuando se solicite. `opadctl` captura el bootloader ROM del ESP32-S3 (`303a:1001`) al encender y flashea el conjunto completo de imágenes por USB.
3. Nunca pulses los botones físicos BOOT o RESET.

Consulta [Recuperación, reflasheo y desvinculación](recovery.md) para ver los procedimientos completos de recuperación, reversión OTA, reconciliación de contadores y desvinculación.
