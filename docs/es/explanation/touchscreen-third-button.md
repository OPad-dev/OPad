---
title: La Pantalla Táctil como Tercer Botón
description: Explicación de la integración del controlador táctil capacitivo CST816S, Quick Retry con dedo en reposo, clasificación de gestos y aislamiento de juego.
category: explanation
source_files:
  - /firmware/main/input/touch_retry.c
  - /firmware/main/input/gesture.c
  - /firmware/main/usb/usb_hid.c
  - /docs/specs/v2-hall-effect-decisions.md
---

En la Waveshare ESP32-S3-Touch-LCD-2, la pantalla táctil capacitiva no está diseñada simplemente como una interfaz gráfica, sino como **el tercer botón** (Regla de Producto 4). Ofrece pulsaciones digitales limpias, estados sostenidos fiables y liberaciones precisas sin sobrecargar la CPU ni degradar la latencia del teclado. <!-- src: /firmware/main/input/touch_retry.c -->

## Hardware & Interrupt Architecture

El subsistema táctil está controlado por un chip capacitivo Hynitron CST816S que se comunica a través de I2C (`CST816_I2C_ADDR = 0x15` a 100 kHz) en GPIO 47 (SDA) y GPIO 48 (SCL), con una línea de interrupción activa en bajo en GPIO 46. <!-- src: /firmware/main/input/touch_retry.c#CST816_I2C_ADDR -->

### Low-CPU Execution Model
- **Cero Sondeo en Reposo**: Cuando no hay ningún dedo tocando la pantalla, la tarea `touch_retry` permanece totalmente inactiva, suspendida en un semáforo de FreeRTOS activado por la interrupción de hardware de GPIO (`TOUCH_IDLE_CHECK_MS = 1000` de reserva). No hay tráfico en el bus I2C mientras el pad no se toque. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_IDLE_CHECK_MS -->
- **Sondeo Dinámico Durante el Contacto**: Una vez que un dedo toca la superficie, el CST816 baja INT a nivel bajo, despertando la tarea. La tarea entra en un bucle de sondeo cada 10 ms (`TOUCH_POLL_MS = 10`) para rastrear el movimiento y la duración del contacto. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_POLL_MS -->
- **Antirrebote Limpio en la Liberación**: Para tolerar lecturas ruidosas de I2C, la detección de liberación requiere dos lecturas vacías consecutivas (~20 ms) antes de soltar el estado de pulsación (`TOUCH_RELEASE_READS = 2`). Un único fallo de lectura transitorio no interrumpirá un toque sostenido. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_RELEASE_READS -->

## Context-Aware Touch Modes

El supervisor táctil clasifica el funcionamiento en tres modos distintos: <!-- src: /firmware/main/input/touch_retry.c#touch_mode_t -->

| Modo | Condición de Activación | Acción Táctil | Acción de Deslizamiento |
|---|---|---|---|
| **`TOUCH_MODE_PLAY`** | El anfitrión transmite activamente un beatmap (`PLAYING`). | El dedo en reposo envía Quick Retry (`` ` `` / `~`). | Los gestos multimedia funcionan sin alterar los informes de teclado. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_PLAY --> |
| **`TOUCH_MODE_MENU`** | Fuera de un beatmap o en la selección de canciones de osu!. | Los toques no activan Quick Retry. | Los deslizamientos controlan volumen, salto de pistas o teclas personalizadas. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_MENU --> |
| **`TOUCH_MODE_RETRY`** | Sin anfitrión conectado o con tosu inactivo. | Cada toque actúa como Quick Retry. | Deslizamientos desactivados. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_MODE_RETRY --> |

### The In-Game Resting Finger (Quick Retry)
En los juegos de ritmo, reiniciar accidentalmente un mapa por un deslizamiento inadvertido o un roce es desastroso. OPad implementa una ventana de inmovilidad: <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US -->
- Cuando comienza un toque durante un mapa, Quick Retry no se envía inmediatamente. El pad observa el recorrido del dedo durante 60 ms (`PLAY_RETRY_STILL_US = 60,000`).
- Si el dedo se desplaza menos de 12 píxeles (`PLAY_RETRY_STILL_PX = 12`) durante este intervalo, se clasifica como una pulsación de reposo intencionada y se activa el código de tecla HID `0x35` (`` ` ``).
- Dado que la acción nativa de Quick Retry en osu! requiere mantener pulsado en lugar de un toque instantáneo, este filtro de inmovilidad de 60 ms no introduce ningún retraso perceptible para el jugador mientras elimina completamente los reinicios accidentales durante movimientos de deslizamiento.

## Swipe Classification & Media Gestures

El rastreo de movimiento está gestionado por la máquina de estados de gestos (`firmware/main/input/gesture.c`): <!-- src: /firmware/main/input/gesture.c#gesture_feed -->

- **Desambiguación de Ejes**: Un gesto comienza en `(origin_x, origin_y)`. El movimiento debe superar `GESTURE_SWIPE_PX` (24 píxeles) con un eje dominando claramente sobre el otro ($2 \cdot |dy| \ge 3 \cdot |dx|$ para vertical, o $2 \cdot |dx| \ge 3 \cdot |dy|$ para horizontal). Los deslizamientos diagonales esperan a un movimiento más claro. <!-- src: /firmware/main/input/gesture.c#gesture_feed -->
- **Asignaciones por Defecto**:
  - **Deslizar Arriba / Abajo**: Ajuste de volumen principal. Cuando osu! está enfocado en el anfitrión de escritorio, `opad-daemon` cambia el volumen interno de osu!; de lo contrario, cambia el volumen del sistema.
  - **Deslizar Izquierda / Derecha**: Navegación de pistas multimedia (Pista Anterior / Pista Siguiente).
- **Protección de Alt en Windows**: Al enviar combinaciones de teclas multimedia o de volumen que involucran teclas modificadoras, Windows interpretará una pulsación y liberación de Alt sin una tecla intermedia como una orden para abrir el menú de la ventana de la aplicación (`SC_KEYMENU`), robándole el foco al juego. Para evitar la pérdida de foco, el pad pulsa `HID_KEY_F24` antes de soltar Alt (`TOUCH_KEY_TAP_MS = 15`). <!-- src: /firmware/main/input/touch_retry.c#HID_KEY_F24 -->

## Gameplay Isolation Invariant

Conforme a la Regla de Producto 4:
1. **Nunca Alt a mitad de mapa**: Los deslizamientos ejecutados en plena partida nunca insertan Alt ni modificadores en el informe del teclado.
2. **Pantalla Limpia**: No se renderiza nada adicional en la pantalla LCD durante el juego activo. Las insignias de confirmación de gestos y las superposiciones de volumen se suprimen hasta que finalice el beatmap.
3. **Pads Hall Effect sin Calibrar**: Si un pad HE no está calibrado, queda bloqueado en su pantalla de calibración (`docs/specs/v2-hall-effect-decisions.md` D7). En este estado, los deslizamientos están desactivados, no se transmiten teclas de teclado y los eventos táctiles se restringen estrictamente a los botones de la interfaz de calibración. <!-- src: /docs/specs/v2-hall-effect-decisions.md -->
