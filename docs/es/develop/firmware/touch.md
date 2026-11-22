---
title: Subsistema de Pantalla Táctil del Firmware
description: Referencia de implementación para el controlador táctil I2C CST816S, ejecución de la tarea táctil, seguimiento de ejes de gestos y renderizado de etiquetas de deslizamiento.
category: reference
source_files:
  - /firmware/main/input/touch_retry.c
  - /firmware/main/input/gesture.c
  - /firmware/main/input/swipe_labels.c
---

El subsistema de pantalla táctil gestiona el controlador táctil capacitivo Hynitron CST816S a través de I2C, implementando el concepto del "tercer botón" del pad (Regla de Producto 4). Se ejecuta en Core 1 a baja prioridad (`touch_retry_task`), manteniendo todo el procesamiento táctil fuera de la ruta crítica de pulsación de teclas. <!-- src: /firmware/main/input/touch_retry.c -->

## Hardware & Bus Configuration

- **Controlador**: Chip táctil capacitivo CST816S (`CST816_I2C_ADDR = 0x15`). <!-- src: /firmware/main/input/touch_retry.c#CST816_I2C_ADDR -->
- **Bus I2C**: Puerto `I2C_NUM_0` funcionando a una frecuencia de reloj de 100 kHz (`CST816_I2C_FREQ_HZ = 100000`). <!-- src: /firmware/main/input/touch_retry.c#CST816_I2C_FREQ_HZ -->
- **Pines**: SDA en GPIO 47, SCL en GPIO 48, línea INT activa en bajo en GPIO 46.
- **Suspensión Automática Desactivada**: El controlador desactiva la suspensión automática del chip (`CST816_REG_AUTO_SLEEP = 0xFE`, valor `0xFF`), bloqueando el controlador en modo activo dinámico para evitar retrasos de reactivación de varios milisegundos. <!-- src: /firmware/main/input/touch_retry.c#CST816_REG_AUTO_SLEEP -->

## Execution & Debounce Loop

La tarea alterna dinámicamente entre la espera por interrupción y el sondeo activo: <!-- src: /firmware/main/input/touch_retry.c -->

1. **Estado de Reposo**: La tarea se suspende esperando un semáforo de FreeRTOS señalado por la interrupción por flanco de bajada en GPIO 46. No hay ciclos de CPU ni transacciones en el bus I2C mientras la pantalla permanece sin tocar. <!-- src: /firmware/main/input/touch_retry.c -->
2. **Toque Activo**: Cuando se dispara INT, la tarea entra en un bucle de sondeo cada 10 ms (`TOUCH_POLL_MS = 10`), leyendo las coordenadas del punto de contacto (`CST816_REG_TOUCH_NUM` y `CST816_REG_TOUCH_XH`). <!-- src: /firmware/main/input/touch_retry.c#TOUCH_POLL_MS -->
3. **Confirmación de Liberación**: Requiere dos lecturas vacías consecutivas (~20 ms) antes de soltar el estado de retención del toque (`TOUCH_RELEASE_READS = 2`). Esto protege contra fallos momentáneos en el bus que puedan interrumpir una retención sostenida de Quick Retry. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_RELEASE_READS -->

## Coordinate Translation & Gesture Engine (gesture.c)

El sensor táctil está montado físicamente en orientación vertical (240x320), mientras que la interfaz de la pantalla se ejecuta en orientación horizontal (320x240): <!-- src: /firmware/main/input/touch_retry.c -->
- Las coordenadas se traducen con `TOUCH_SWAP_XY = 1` y `TOUCH_MIRROR_Y = 1` para que las coordenadas físicas de la pantalla coincidan con la orientación del renderizado de la GUI. <!-- src: /firmware/main/input/touch_retry.c#TOUCH_SWAP_XY -->
- **Rastreador de Gestos (`gesture_tracker_t`)**:
  - `gesture_begin()`: Fija el punto de origen inicial del contacto `(origin_x, origin_y)`. <!-- src: /firmware/main/input/gesture.c#gesture_begin -->
  - `gesture_feed()`: Analiza el movimiento. Requiere un desplazamiento $> 24$ píxeles (`GESTURE_SWIPE_PX = 24`). Desambigua los ejes: los deslizamientos verticales requieren $2 \cdot |dy| \ge 3 \cdot |dx|$; los horizontales requieren $2 \cdot |dx| \ge 3 \cdot |dy|$. Las diagonales ambiguas esperan a un mayor desplazamiento. <!-- src: /firmware/main/input/gesture.c#gesture_feed -->

## Swipe Labels & UI Feedback (swipe_labels.c)

Cuando se ejecuta una acción de deslizamiento fuera de la partida, la interfaz muestra una insignia superpuesta que indica el icono de la acción y la etiqueta de texto correspondiente (tabla `SWIPE_LABELS` en `firmware/main/input/swipe_labels.c`): <!-- src: /firmware/main/input/swipe_labels.c#SWIPE_LABELS -->

- Los atajos admitidos incluyen Mapa Aleatorio (`F2`), Mods (`F1`), Opciones (`F3`) y Pista Multimedia Anterior/Siguiente.
- Durante los beatmaps activos (`PLAYING`), todas las insignias en pantalla se suprimen para garantizar cero sobrecarga de visualización y cero distracciones visuales.
