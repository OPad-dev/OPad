---
title: Primera conexión
description: Conecta OPad y comprueba que funciona como teclado USB, sin instalar ningún software ni controlador.
sidebar:
  order: 2
---

OPad es un teclado USB estándar. No necesita controlador, daemon ni app para escribir, así que puedes probarlo enseguida en cualquier PC con Linux o Windows 10/11. <!-- src: /AGENTS.md#1. Product rules -->

## Conéctalo

1. Conecta un cable USB-C **de datos** al puerto USB-C de la parte trasera del teclado. (En un teclado montado por ti sin carcasa, usa el propio puerto USB-C de la placa Waveshare). <!-- src: /hardware/3d/custom_case/V3/README.md -->
2. Conecta el otro extremo a tu PC, preferiblemente directo al PC y no a través de un hub.
3. La pantalla se enciende y el teclado aparece en tu PC como teclado.

> [!NOTE]
> Un teclado Hall Effect que nunca se ha calibrado muestra en su lugar la pantalla de calibración y no envía ninguna tecla hasta que lo calibres. Consulta [Calibrar interruptores Hall Effect](../guides/calibrate-hall-effect.md). <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## Prueba las teclas

Abre cualquier editor de texto:

1. Pulsa **Key 1**: aparece `z`.
2. Pulsa **Key 2**: aparece `x`.
3. Pulsa **Key 3**: aparece `c`. (Un teclado cableado a mano solo tiene Key 1 y Key 2).
4. Toca la pantalla y mantén el dedo: `` ` `` (acento grave) se mantiene pulsado hasta que lo levantes. Sin la app de escritorio, cada toque es la tecla Quick Retry de osu!. <!-- src: /firmware/main/input/touch_retry.c#touch_mode_now -->

Estos son los valores predeterminados: <!-- src: /firmware/main/usb/usb_hid.c --> <!-- src: /firmware/main/config/device_config.h -->

| Control | Tecla predeterminada (US QWERTY) | Uso HID |
|---|---|---|
| Key 1 | `z` | `0x1D` |
| Key 2 | `x` | `0x1B` |
| Key 3 | `c` | `0x06` |
| Pantalla táctil | `` ` `` (Quick Retry) | `0x35` |

El teclado envía posiciones de tecla, no letras, así que con otra distribución de teclado puedes ver caracteres distintos (por ejemplo, `w` en lugar de `z` en AZERTY). Es lo esperado; consulta [Solución de problemas §1](../troubleshooting.md#1-distribuciones-de-teclado-qwerty-azerty-qwertz).

## Comprueba cómo lo ve tu PC

No lo necesitas para jugar. Ayuda cuando algo no funciona.

OPad aparece como un único dispositivo USB con dos interfaces HID (el teclado y los controles multimedia) y un puerto serie (CDC-ACM) que usa la app de escritorio. Las dos interfaces HID se sondean cada 1 ms (1000 Hz). <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_config_desc -->

| Campo | Valor |
|---|---|
| ID USB | `303a:8415` (`303a:1001` es el bootloader integrado del ESP32-S3, que solo ves mientras se flashea) |
| Fabricante | `GFerreiroS` |
| Producto | `OPad ESP32-S3` |
| Número de serie | `OPAD-` seguido de 12 dígitos hexadecimales de la dirección MAC de la placa |
<!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_device_desc --> <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_string_desc --> <!-- src: /firmware/main/usb/usb_descriptors.c#usb_descriptors_init -->

**Linux:**

```bash
lsusb -d 303a:8415
```

El puerto serie del teclado aparece como `/dev/ttyACM*`.

**Windows:** abre el **Device Manager** (Administrador de dispositivos). Las teclas del teclado aparecen en **Keyboards** y su puerto serie en **Ports (COM & LPT)** como **OPad (COMx)**; ese nombre se lo da el propio descriptor USB del teclado. <!-- src: /firmware/main/usb/usb_descriptors.c#opad_usb_ms_os_20_desc -->

## Siguiente paso

El teclado ya está listo para jugar. Para tener datos del mapa en vivo en la pantalla, el diseñador de pantalla, los deslizamientos y las actualizaciones de firmware, sigue con [Instalar la app de escritorio](install-the-app.md).
