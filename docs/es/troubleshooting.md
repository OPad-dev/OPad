---
title: Guía de solución de problemas
description: Problemas habituales con OPad, integración de escritorio, acceso a puertos, permisos de tosu y telemetría en pantalla.
category: reference
source_files:
  - /desktop/daemon/src/device_actor.rs
  - /desktop/gui/src/diagnostics.rs
  - /desktop/crates/opad-tosu/src/lib.rs
  - /firmware/main/diag/diag.c
  - /packaging/linux/udev/70-opad.rules
---

Esta guía cubre preguntas frecuentes y pasos para la resolución de problemas con el teclado OPad, la integración de escritorio, las asignaciones de teclas y la telemetría en pantalla en Linux y Windows.

---

## 1. Distribuciones de teclado y mapeo de scancodes (QWERTY, AZERTY, QWERTZ)

### Cómo funcionan los scancodes en OPad
OPad es un teclado compuesto USB HID a 1000 Hz. Los teclados USB HID **no** transmiten letras localizadas (como "Z" o "W"); transmiten **USB HID Key Usage IDs** (scancodes de hardware). La distribución de teclado configurada en el sistema operativo traduce estos identificadores en el carácter correspondiente según tu idioma.

Scancodes predeterminados del firmware:
* **Tecla 1**: `0x1D` (`HID_USAGE_KEY_KEYBOARD_Z`)
* **Tecla 2**: `0x1B` (`HID_USAGE_KEY_KEYBOARD_X`)

### Comportamiento en distribuciones no QWERTY

| Distribución en el sistema operativo | La Tecla 1 física envía | El SO interpreta como | Acción recomendada |
|---|---|---|---|
| **US / UK QWERTY** | Usage `0x1D` | **`Z`** | Ninguna (predeterminado `Z` / `X` en osu!) |
| **AZERTY francés** | Usage `0x1D` | **`W`** | En osu!, asigna la tecla izquierda a `W` (o reasigna en OPad) |
| **QWERTZ alemán** | Usage `0x1D` | **`Y`** | En osu!, asigna la tecla izquierda a `Y` (o reasigna en OPad) |
| **Dvorak / Colemak** | Usage `0x1D` | Según distribución | Reasigna en osu! o en OPad |

### Ajuste de la asignación de teclas
Tienes dos opciones:
1. **En osu! / osu!lazer (La más rápida)**: Abre **Opciones → Asignación de teclas** en osu! y pulsa la Tecla 1 y la Tecla 2 en tu OPad para asignar el scancode que el teclado transmita.
2. **En OPad**: abre la aplicación de escritorio, ve a **Ajustes → Teclas**, y elige la letra que enviará cada tecla. El teclado la guarda en memoria, manteniéndose en cualquier ordenador.

---

## 2. Linux Yama ptrace Scope y telemetría de memoria con tosu

### El problema
OPad muestra en su pantalla integrada la precisión en tiempo real, el combo, la tasa de inestabilidad (UR) y los detalles del beatmap utilizando `tosu`. En Linux, `tosu` lee la memoria del proceso `osu!` en ejecución (tanto si se ejecuta mediante Wine/Proton como en Linux nativo con `osu!lazer`).

Las distribuciones modernas de Linux (Ubuntu, Debian, Fedora, Arch) protegen la memoria de los procesos mediante el **Módulo de Seguridad Linux Yama** a través de `/proc/sys/kernel/yama/ptrace_scope`:
* **Scope 0**: Permisos clásicos de ptrace (los procesos con el mismo UID pueden inspeccionarse entre sí).
* **Scope 1 (Predeterminado en Ubuntu/Debian)**: ptrace restringido (los procesos solo pueden inspeccionar a sus hijos directos).
* **Scope 2**: ptrace solo para administradores (únicamente procesos con `CAP_SYS_PTRACE` pueden leer la memoria).
* **Scope 3**: Ningún ptrace permitido hasta reiniciar el equipo.

Cuando `ptrace_scope` está configurado en `1` o superior, `tosu` no puede acoplarse a `osu!` y la telemetría en vivo en la pantalla de OPad permanecerá en pausa o mostrará cero.

### Solución 1: Capacidad de archivo (Recomendado)
Otorgar `CAP_SYS_PTRACE` específicamente al binario `tosu` le permite leer la memoria de los procesos sin comprometer la seguridad global del sistema:

```bash
# Si se instaló mediante paquete:
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu

# Si se instaló localmente (~/.local):
sudo setcap cap_sys_ptrace=eip ~/.local/lib/opad/tosu/tosu
```
> [!NOTE]
> Los paquetes `.deb` y `.rpm` aplican esta capacidad automáticamente durante su instalación. El paquete **AppImage** no puede hacerlo: se ejecuta desde un montaje con la opción `nosuid`, que ignora las capacidades de archivo. El **paquete de Arch** ejecuta tosu con el `node` del sistema, y otorgar a `node` esta capacidad se la concedería a cualquier programa Node. En esos dos casos, utiliza la Solución 2. El comando `opadctl setup` y la página de Diagnósticos te indicarán en qué caso te encuentras.

### Solución 2: Ámbito ptrace en todo el sistema
Si prefieres permitir ptrace en toda tu sesión de usuario:

**Temporal (hasta reiniciar):**
```bash
echo 0 | sudo tee /proc/sys/kernel/yama/ptrace_scope
```

**Persistente (entre reinicios):**
Crea o edita `/etc/sysctl.d/10-ptrace.conf`:
```ini
kernel.yama.ptrace_scope = 0
```
A continuación, aplica los cambios:
```bash
sudo sysctl --system
```

---

### tosu se cierra de inmediato con un error `GLIBC_2.xx not found`
El calculador de pp precompilado de la versión oficial de tosu (`@tosuapp/lazer-calculator`) requiere glibc 2.38, por lo que una compilación estándar de tosu fallará al iniciarse en Ubuntu 22.04, Debian 12 y RHEL 9. Los paquetes oficiales de OPad lo recompilan (así como el complemento nativo de tosu) para glibc 2.28, de modo que el tosu integrado funciona en todas las distribuciones compatibles. Solo verás este error si utilizas un tosu compilado por otros medios: uno propio (`OPAD_TOSU_PATH`) o mediante `make tosu TOSU_PORTABLE=0`.

## 3. Permisos USB y acceso al puerto serie (Linux)

### Síntoma: `Permission denied (os error 13)` o dispositivo no detectado
En Linux, los usuarios estándar no tienen acceso directo a los dispositivos serie USB (`/dev/ttyACM*`) ni a los nodos de entrada de forma predeterminada.

### Solución
Los paquetes `.deb` y `.rpm` instalan la regla necesaria. La AppImage ofrece un botón de **Instalar** en su primera ejecución (o ejecuta `./opad-x86_64.AppImage install-udev`). Desde una copia del código fuente:
```bash
sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```
Esta regla también permite a la página de Diagnósticos leer los eventos de pulsación del teclado.

### Interferencia de ModemManager
ModemManager sondea nuevos dispositivos serie enviando comandos Hayes `AT`. Esto puede corromper el apretón de manos del protocolo CDC. La regla oficial `70-opad.rules` incluye:
```udev
ATTRS{idVendor}=="303a", ATTRS{idProduct}=="4001|1001", ENV{ID_MM_DEVICE_IGNORE}="1"
```
Asegúrate de estar utilizando la versión más reciente de `70-opad.rules`.

---

## 4. Windows SmartScreen y permisos

### Windows Defender SmartScreen
El instalador de OPad (`opad-setup.exe`) se genera a partir de este repositorio mediante el flujo de publicación de GitHub Actions y todavía no cuenta con firma de código. Si Windows muestra "Windows protegió su PC":
1. Haz clic en **Más información**.
2. Haz clic en **Ejecutar de todas formas**.
3. Puedes verificar el hash del instalador comparándolo con el publicado en `SHA256SUMS` en la página de GitHub Releases.

### Falsos positivos de antivirus en `tosu.exe`
Debido a que `tosu` lee la memoria virtual de otro proceso (`osu!.exe`), algunas heurísticas de antivirus demasiado agresivas pueden marcar o poner en cuarentena `tosu.exe`. Si la telemetría deja de actualizarse en Windows:
1. Revisa la Cuarentena / Historial de protección de tu antivirus.
2. Añade a la lista de exclusiones `%LOCALAPPDATA%\Programs\opad\tosu\tosu.exe`.

---

## 5. Recuperación y reflasheo del firmware

Si el teclado deja de responder, se corrompe o no se enumera como un dispositivo USB OPad:
1. Ejecuta el comando de recuperación no interactivo:
   ```bash
   opadctl recover firmware/build
   ```
2. Cuando `opadctl` lo solicite, simplemente desconecta y vuelve a conectar el cable USB. La herramienta captura la ventana del bootloader ROM del ESP32-S3 durante el encendido y recupera el teclado a través de USB.
3. El teclado nunca requiere pulsar físicamente los botones BOOT o RESET.

Consulta [docs/es/recovery.md](recovery.md) para conocer los pasos completos de recuperación del hardware y los mecanismos de reversión.
