---
title: Primera conexión
description: Cómo conectar OPad a un ordenador anfitrión, verificar la enumeración como teclado USB HID a 1000 Hz sin software y probar las asignaciones predeterminadas.
---

OPad funciona de fábrica como un teclado USB HID autónomo a 1000 Hz sin software de escritorio ni controladores instalados. No hace falta ningún daemon en segundo plano, aplicación complementaria ni controlador de terceros para escribir o jugar juegos de ritmo. <!-- src: /AGENTS.md --> <!-- src: /README.md -->

## Conectar el teclado

1. Conecta un cable USB Tipo-C de datos a la placa Waveshare ESP32-S3 (o al puerto USB-C trasero en una carcasa OPad V3). <!-- src: /hardware/pcb/V3/README.md -->
2. Conecta el otro extremo a un puerto USB de tu PC.
3. El ESP32-S3 se enciende, inicializa la pantalla LCD de 2.0 pulgadas y enumera por USB. (Nota: si hay instalado un módulo Hall Effect sin calibrar, la pantalla se bloquea en el aviso de calibración y suprime las teclas hasta que se calibre; consulta [Calibración de interruptores Hall Effect](../guides/calibrate-hall-effect.md).) <!-- src: /firmware/main/ui/he_cal_ui.c -->

## Detalles de enumeración USB

OPad enumera como un dispositivo USB compuesto con dos interfaces HID a 1000 Hz (`bInterval = 1`) (teclado + multimedia/Key 2) y una interfaz serie CDC-ACM: <!-- src: /firmware/main/usb/usb_descriptors.c -->

- **Vendor ID (VID)**: `0x303A` (Espressif)
- **Product ID (PID)**: `0x4001` (firmware de aplicación OPad; `0x1001` es el bootloader ROM del ESP32-S3)
- **Manufacturer String**: `GFerreiroS` <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_string_desc -->
- **Product String**: `OPad ESP32-S3` <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_string_desc -->
- **Serial String**: `OPAD-XXXXXXXXXXXX` (12 dígitos hexadecimales en mayúsculas derivados de la dirección MAC Wi-Fi STA del ESP32-S3) <!-- src: /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

### Verificación en Linux
Ejecuta `lsusb`:
```bash
lsusb -d 303a:4001
```
Comprueba los mensajes del núcleo con `dmesg`:
```bash
dmesg | grep -i opad
```
Deberías ver enumeradas las interfaces de teclado HID `OPad ESP32-S3`, controles multimedia y serie CDC-ACM (`/dev/ttyACM*`). <!-- src: /firmware/main/usb/usb_descriptors.c -->

### Verificación en Windows
1. Abre el **Administrador de dispositivos** (**Device Manager**).
2. Despliega **Teclados** (**Keyboards**): aparecen entradas estándar **HID Keyboard Device** usando el controlador integrado `hidclass.sys`. <!-- src: /README.md -->
3. Despliega **Puertos (COM y LPT)** (**Ports (COM & LPT)**): aparece **OPad (COMx)** usando el controlador integrado `usbser.sys` (Windows lee la propiedad `FriendlyName = "OPad"` del descriptor MS OS 2.0 de OPad). <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_ms_os_20_desc -->

## Asignaciones de teclas predeterminadas

De fábrica, OPad utiliza estos códigos HID predeterminados: <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/main/input/keypad.c -->

| Control | Conexión de hardware | Uso HID por defecto | Tecla US QWERTY |
|---|---|---|---|
| **Key 1** | Cableado manual: `GPIO 14` (P2 Pin 11) / V3 MX: `GPIO 10` / V3 HE: ADS7028 `AIN5` | `0x1D` | `z` <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h --> |
| **Key 2** | Cableado manual: `GPIO 9` (P2 Pin 12) / V3 MX: `GPIO 7` / V3 HE: ADS7028 `AIN4` | `0x1B` | `x` <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h --> |
| **Key 3** (módulos V3 de 3 teclas) | V3 MX: `GPIO 6` / V3 HE: ADS7028 `AIN3` | `0x06` | `c` <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h --> |
| **Pantalla táctil** | Táctil capacitivo CST816S | `0x35` | `` ` `` / `~` (Quick Retry en modo autónomo o dentro de un mapa) <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/main/input/touch_retry.c --> |

### Prueba rápida de escritura
Abre cualquier editor de texto:
1. Pulsa Key 1: aparece `z` (o la tecla equivalente en distribuciones no QWERTY; consulta [Solución de problemas §1](../troubleshooting.md)).
2. Pulsa Key 2: aparece `x`.
3. Pulsa Key 3 (en módulos V3 de 3 teclas): aparece `c`.
4. Toca y mantén pulsada la pantalla LCD (sin daemon ni `tosu` conectados): `` ` `` se mantiene pulsada mientras el dedo sigue apoyado y se suelta limpiamente al levantarlo. <!-- src: /firmware/main/input/touch_retry.c -->

Para habilitar la telemetría HUD de osu! en vivo, la personalización del diseño de pantalla, las acciones de deslizamiento o los ajustes Hall Effect, continúa con [Instalación de la suite de escritorio](install-the-app.md).
