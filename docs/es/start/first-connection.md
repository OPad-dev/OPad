---
title: Primera conexión
description: Conectar OPad al ordenador host, verificar la enumeración del teclado USB HID a 1000 Hz sin software y realizar pruebas de asignación de teclas por defecto.
---

El teclado está diseñado para funcionar como un teclado USB independiente y sin necesidad de controladores nada más conectarlo a un ordenador (Regla de producto 2). No se requiere ningún demonio en segundo plano, aplicación complementaria ni controladores de dispositivos especializados para escribir o jugar a juegos de ritmo. <!-- src: /AGENTS.md -->

## Conectar el teclado

1. Conecta un cable de datos USB Type-C al puerto Type-C de la placa Waveshare ESP32-S3 (o al puerto Type-C trasero si utilizas la caja y la placa portadora OPad V3). <!-- src: /README.md -->
2. Conecta el extremo opuesto directamente a un puerto USB 2.0 o 3.0 de tu ordenador.
3. Tras la conexión:
   - El ESP32-S3 se enciende de inmediato.
   - La pantalla LCD de 2,0 pulgadas se inicializa e ilumina con la pantalla de inicio predeterminada de OPad.
   - El dispositivo se enumera a través de USB en 1–2 segundos.

## Verificación del teclado USB HID

OPad se enumera en el bus del host con descriptores USB personalizados (`firmware/main/usb/usb_descriptors.c`): <!-- src: /firmware/main/usb/usb_descriptors.c -->
- **Vendor ID (VID)**: `0x303A` (Espressif Systems)
- **Product ID (PID)**: `0x4001` (OPad Application Runtime)
- **Cadena de fabricante**: `"OPad"`
- **Cadena de producto**: `"OPad Keypad"`
- **Cadena de serie**: Formato `OPAD-<MAC_ADDRESS>` derivado de la dirección MAC de fábrica del ESP32-S3. <!-- src: /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

### Verificación en Linux
Abre un terminal y ejecuta `lsusb`:
```bash
lsusb -d 303a:4001
```
La salida confirma la enumeración:
```
Bus 001 Device 024: ID 303a:4001 Espressif OPad Keypad
```
Inspecciona los registros del kernel con `dmesg`:
```bash
dmesg | grep -i opad
```
La salida muestra dos interfaces de teclado HID y una interfaz serie CDC-ACM inicializadas a una frecuencia de sondeo de 1000 Hz ($bInterval = 1$). <!-- src: /firmware/main/usb/usb_descriptors.c -->

### Verificación en Windows
1. Abre el **Administrador de dispositivos**.
2. Despliega **Teclados**: Verás entradas estándar de **Dispositivo de teclado HID** registradas por Windows.
3. Despliega **Puertos (COM y LPT)**: Verás un **Dispositivo serie USB (COMx)** correspondiente a la interfaz de control CDC del teclado.

## Asignación de teclas por defecto

De fábrica, OPad viene preconfigurado con las teclas estándar para juegos de ritmo: <!-- src: /firmware/main/input/keypad.c -->

| Control | Pin por defecto | Uso USB HID | Carácter generado |
|---|---|---|---|
| **Tecla 1** | GPIO 14 (Cabecera P2 Pin 11) | `0x1D` | `z` <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY1_GPIO --> |
| **Tecla 2** | GPIO 9 (Cabecera P2 Pin 12) | `0x1B` | `x` <!-- src: /firmware/main/input/keypad.c#KEYPAD_DEFAULT_KEY2_GPIO --> |
| **Tecla 3** (Hall Effect V3) | GPIO 15 (Cabecera P2 Pin 7) | `0x06` | `c` <!-- src: /firmware/main/usb/usb_hid.c --> |
| **Pantalla táctil** | Superficie CST816S | `0x35` | `` ` `` / `~` (Reintento rápido de osu!) <!-- src: /firmware/main/usb/usb_hid.c --> |

### Prueba rápida de escritura
Abre cualquier editor de texto o barra de búsqueda del navegador:
1. Pulsa el interruptor físico conectado a la Tecla 1: aparecerá el carácter `z` en pantalla.
2. Pulsa el interruptor físico conectado a la Tecla 2: aparecerá el carácter `x` en pantalla.
3. Mantén pulsada la pantalla táctil LCD: se introducirá el carácter `` ` ``.

El teclado ya está completamente operativo como un dispositivo competitivo de latencia ultrabaja. Para añadir superposiciones de telemetría de canciones o personalizar los puntos de actuación, continúa con la instalación de la suite de escritorio. <!-- src: /README.md -->
