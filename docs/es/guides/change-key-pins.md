---
title: Cambio de pines de las teclas
description: Cómo reconfigurar las asignaciones de pines GPIO para interruptores mecánicos mediante opadctl o la interfaz gráfica, restricciones de pines y reglas de validación.
category: how-to
source_files:
  - /README.md
  - /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h
  - /firmware/main/config/config_validate.c
  - /firmware/main/config/device_config.c
  - /protocol/opad.proto
---

Por defecto, OPad asigna la Tecla 1 al GPIO 14 (Cabecera P2 Pin 11) y la Tecla 2 al GPIO 9 (Cabecera P2 Pin 12). Si necesitas evitar pines específicos o adaptarte a cajas personalizadas, puedes reconfigurar las asignaciones GPIO de los interruptores mediante `opadctl` o `opad-gui`. <!-- src: /firmware/boards/waveshare_esp32s3_touch_lcd_2/board_pins.h -->

> [!NOTE]
> La reasignación de pines de las teclas se aplica únicamente a interruptores mecánicos cableados a mano. Las placas portadoras modulares V3 fijan sus asignaciones de pines en las pistas de la PCB; en esos teclados, las opciones de configuración de pines se ocultan automáticamente. <!-- src: /desktop/crates/opad-model/src/lib.rs#key_pins_configurable -->

## Pines GPIO permitidos

Solo pueden utilizarse pines que admitan resistencias pull-up internas y no entren en conflicto con los periféricos integrados (SPI de la pantalla ST7789, I2C táctil CST816S, USB D+/D- o UART de depuración): <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED -->

| Posición en cabecera | Número GPIO | Función por defecto | ¿Utilizable para interruptor? |
|---|---|---|---|
| P2 Pin 11 | **GPIO 14** | Tecla 1 por defecto | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 12 | **GPIO 9** | Tecla 2 por defecto | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 7 | **GPIO 15** | Tecla 3 (HE V3) / Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 8 | **GPIO 13** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 9 | **GPIO 11** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P2 Pin 10 | **GPIO 12** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 1 | **GPIO 2** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 2 | **GPIO 4** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 3 | **GPIO 6** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 9 | **GPIO 7** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 10 | **GPIO 10** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 4 | **GPIO 16** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 6 | **GPIO 18** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |
| P1 Pin 7 | **GPIO 21** | Reserva | Sí <!-- src: /firmware/main/config/config_validate.c#KEY_GPIO_ALLOWED --> |

### Pines no permitidos
- **GPIO 19 y 20**: Líneas de datos nativas USB de velocidad completa (`D-` y `D+`).
- **GPIO 43 y 44**: Consola serie y bootloader ROM de UART0 (`TX0` y `RX0`).
- **GPIO 47 y 48**: Bus I2C para el controlador táctil y el sensor IMU.
- **GPIO 17**: Línea de apagado de la cámara por hardware (`CAM_PWDN`), conectada a una resistencia pull-down de 10 kΩ a GND.
- **GPIO 8**: Línea del divisor ADC para la detección de módulos.

## Cambiar los pines mediante opadctl

Para reasignar pines desde la terminal, utiliza `opadctl config set`: <!-- src: /desktop/cli/src/main.rs -->
```bash
# Asignar Tecla 1 a GPIO 15 y Tecla 2 a GPIO 13
opadctl config set key1-gpio 15
opadctl config set key2-gpio 13
```

Valida las nuevas asignaciones consultando el dispositivo:
```bash
opadctl status
```
La salida reflejará las asignaciones de pines GPIO activas.

## Cambiar los pines mediante la interfaz gráfica

1. Abre `opad-gui`.
2. Dirígete a la página **Dispositivo**.
3. Localiza los selectores desplegables **Pin de Tecla 1** y **Pin de Tecla 2**.
4. Selecciona la posición de cabecera deseada (p. ej., `GPIO15 (P2 pin 7)`).
5. Haz clic en **Aplicar cambios**. El demonio transmitirá `SetConfig` (`opad.proto`) y aplicará inmediatamente la nueva configuración en RAM sin requerir reiniciar el teclado. <!-- src: /protocol/opad.proto#SetConfig -->

## Probar la actuación con detect-pin

Para asegurarte de que el interruptor físico está cableado al pin correcto antes de jugar:
```bash
opadctl detect-pin 15
```
Pulsa el interruptor físico conectado al GPIO 15. El teclado informará del cierre de contacto detectado en el terminal.
