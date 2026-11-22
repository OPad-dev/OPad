---
title: Ingeniería de Latencia
description: Explicación detallada del cauce de entrada sub-milisegundo de OPad, endpoints duales USB HID, algoritmos de antirrebote anticipado y telemetría de latencia.
category: explanation
source_files:
  - /firmware/main/input/debounce.c
  - /firmware/main/input/keypad.c
  - /firmware/main/usb/usb_hid.c
  - /firmware/main/input/latency_stats.c
---

En los juegos de ritmo como *osu!*, la precisión temporal se evalúa en márgenes de milisegundos de un solo dígito. Cualquier fluctuación (jitter) en la latencia o retraso en la transmisión de entrada se traduce directamente en una menor estabilidad en los impactos (Unstable Rate / UR). OPad trata la latencia como su invariante primordial: **la latencia siempre gana** (Regla de Producto 1). Cada componente de la ruta crítica de teclas está optimizado para entregar los cambios de estado de los interruptores al anfitrión USB en menos de 1 milisegundo. <!-- src: /firmware/main/usb/usb_hid.c -->

## Dual USB HID Endpoints

Un endpoint de teclado USB HID estándar a 1000 Hz es sondeado por el sistema operativo anfitrión una vez cada 1.0 ms. Bajo una arquitectura convencional de un solo endpoint:
- Si se activa Key 1, se pone en cola un informe de teclado HID en el endpoint.
- Si Key 2 se activa 200 µs después (algo común durante ráfagas rápidas de dedos que superan los 200–300 BPM), el endpoint no puede aceptar el segundo informe hasta que el anfitrión haya sondeado y confirmado el primer informe (hasta 1 ms más tarde).
- Esto introduce hasta 1 ms de fluctuación artificial de latencia entre pulsaciones alternas de dedos.

### OPad's Split Endpoint Architecture
OPad resuelve este cuello de botella dividiendo las dos teclas principales de juego entre dos endpoints físicos independientes de USB HID: <!-- src: /firmware/main/usb/usb_hid.c#hid_link_t -->

| Tecla | Endpoint / Interfaz | ID de Informe HID | Notas |
|---|---|---|---|
| **Key 1** | Endpoint IN 1 (Interfaz Teclado) | Informe de Teclado de Arranque Estándar | Comparte endpoint con Quick Retry táctil (`~`). <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY1_TOUCH --> |
| **Key 2** | Endpoint IN 2 (Interfaz Consumidor / Media) | Informe de Teclado Personalizado | Comparte endpoint con gestos multimedia fuera de partida. <!-- src: /firmware/main/usb/usb_hid.c#LINK_KEY2 --> |
| **Key 3** | Emparejamiento Dinámico (`usb_hid_set_key3_link`) | Emparejado con Key 1 o Key 2 | Se enruta al endpoint que no esté siendo utilizado por la segunda tecla de juego activa. <!-- src: /firmware/main/usb/usb_hid.c --> |

Dado que Key 1 y Key 2 residen en endpoints USB separados, Key 2 nunca espera a que se complete la transferencia USB de Key 1. Ambas teclas pueden enviar informes al anfitrión dentro del mismo intervalo de sondeo USB de 1 ms sin bloquearse mutuamente. <!-- src: /firmware/main/usb/usb_hid.c -->

### Lockless Sequence Tracking (hid_link_t)
Para evitar condiciones de carrera entre Core 0 (tarea de teclado) y Core 1 (tarea táctil) sin utilizar mutexes bloqueantes en la ruta de entrada, cada enlace de endpoint HID utiliza números de secuencia atómicos: <!-- src: /firmware/main/usb/usb_hid.c#hid_link_t -->
- `change_seq`: Incrementado atómicamente por cualquier tarea que registre un cambio de tecla.
- `sent_seq`: Actualizado cuando un informe USB se pone en cola exitosamente en el búfer de TinyUSB.
- `pending_edge_us`: Marca de tiempo en microsegundos de alta resolución de la transición física del interruptor, utilizada para calcular la latencia de hardware de extremo a extremo.

## Eager Debouncing Algorithm

Los interruptores mecánicos sufren de rebote de contacto, donde las láminas metálicas oscilan entre abierto y cerrado durante 1 a 5 ms tras la actuación física. <!-- src: /firmware/main/input/debounce.c -->

Los teclados convencionales emplean **antirrebote por aplazamiento (deferral debouncing)**: cuando se detecta un flanco de contacto, el controlador espera 5 ms para asegurarse de que la señal se estabilice antes de transmitir la pulsación a USB. Esto añade 5 ms inevitables de latencia de entrada a cada pulsación.

OPad implementa **antirrebote anticipado (eager debouncing)** (`debounce_step` en `firmware/main/input/debounce.c`): <!-- src: /firmware/main/input/debounce.c#debounce_step -->

```
Interruptor Físico:  [Contacto Inicial] ~~~rebote~~~ [Contacto Sólido] ~~~rebote~~~ [Liberación]
Nivel de Señal:      0 -> 1 -> 0 -> 1 -> 1 -> 1 -> 1 -> 1 -> 0 -> 1 -> 0 -> 0
Estado Anticipado:   [EMITE PULSACIÓN INMEDIATA] ---- Periodo de Bloqueo ----> [EMITE LIBERACIÓN]
Latencia Añadida:    0.0 ms
```

### Eager Debounce Operation
1. **Actuación Inmediata**: Cuando una interrupción por flanco pasa de liberado a presionado fuera de una ventana de bloqueo, el evento de pulsación se emite inmediatamente a Core 0 y TinyUSB (`DEBOUNCE_ACTION_PRESS`). La latencia física desde el cierre de contacto hasta la inserción en la cola USB es inferior a 20 microsegundos. <!-- src: /firmware/main/input/debounce.c#debounce_step -->
2. **Periodo de Bloqueo**: Se inicia un temporizador interno (`lockout_end_us = now_us + lockout_us`, por defecto 5,000 µs). Los rebotes eléctricos posteriores que ocurren durante esta ventana de bloqueo se ignoran. <!-- src: /firmware/main/input/debounce.c#debounce_step -->
3. **Remuestreo**: Al finalizar el periodo de bloqueo, se vuelve a muestrear el pin del interruptor. Si el interruptor permanece físicamente presionado, el estado se mantiene presionado. Cuando se libera, se emite un evento de liberación inmediato y se aplica una ventana de bloqueo de liberación.

## Microsecond Latency Telemetry

OPad rastrea la latencia de entrega de físico a USB en tiempo real directamente dentro del firmware: <!-- src: /firmware/main/input/latency_stats.c -->

- **Temporizador de Hardware**: Las transiciones se marcan utilizando el temporizador de alta resolución del ESP32-S3 (`esp_timer_get_time()`, precisión de microsegundos). <!-- src: /firmware/main/input/latency_stats.c#latency_stats_record -->
- **Cubetas de Histograma**: El firmware agrupa las mediciones de latencia en 500 cubetas atómicas de 10 µs cada una (`BUCKET_US = 10`, abarcando de 0 a 5,000 µs). <!-- src: /firmware/main/input/latency_stats.c#BUCKET_COUNT -->
- **Detección de Valores Atípicos**: Cualquier informe que tarde más de 1,000 µs (1.0 ms) incrementa `s_outlier_count` y registra `s_outlier_max_us`, proporcionando visibilidad instantánea sobre retrasos de planificación del host USB o congestión en el bus. <!-- src: /firmware/main/input/latency_stats.c#latency_stats_record -->

## Memory & Execution Invariants

Para eliminar picos de latencia causados por recolección de basura, asignación de memoria o contención en el bus:
- **Cero Asignaciones en el Heap en Core 0**: No existen llamadas a `malloc()` ni `free()` en la ruta de ejecución de teclado o USB. Todas las estructuras, estados de antirrebote y búferes se asignan estáticamente.
- **Ubicación en IRAM**: Las funciones críticas en el tiempo (`debounce_step`, `latency_stats_record`, rutinas de servicio de interrupción de GPIO) están decoradas con `IRAM_ATTR`, fijando las instrucciones máquina en la RAM rápida interna para evitar paradas por fallos de caché de instrucciones. <!-- src: /firmware/main/input/debounce.c#debounce_step --> <!-- src: /firmware/main/input/latency_stats.c#latency_stats_record -->
- **Cero Acceso a Flash Durante la Partida**: El acceso a la caché flash detiene la CPU durante decenas de milisegundos en operaciones de borrado y escritura. El firmware desactiva estrictamente las confirmaciones a NVS mientras se juegan mapas (`PLAYING` / `COOLDOWN`).
