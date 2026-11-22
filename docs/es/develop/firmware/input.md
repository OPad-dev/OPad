---
title: Subsistema de Entrada del Firmware
description: Referencia de implementación para el cauce de entrada del firmware, antirrebote de interruptores mecánicos, muestreo ADC de Hall Effect y despacho unificado de eventos de teclas.
---

El subsistema de entrada captura los contactos físicos de los interruptores y las transiciones de flujo magnético, aplica algoritmos de antirrebote o Rapid Trigger, actualiza los contadores de pulsaciones monotónicos y envía informes HID a los endpoints USB. Todo el código de entrada se ejecuta estrictamente en Core 0 con `configMAX_PRIORITIES - 1`. <!-- src: /firmware/main/input/keypad.c -->

## Mechanical Keypad Pipeline (keypad.c & debounce.c)

Para interruptores mecánicos, el cauce consta de interrupciones GPIO disparadas por flanco emparejadas con un antirrebote anticipado (eager debouncing): <!-- src: /firmware/main/input/debounce.c -->

- **Gestión de Interrupciones**: Los cambios de estado de pin en Key 1 (GPIO 14) y Key 2 (GPIO 9) activan una interrupción GPIO sensible a flancos (`gpio_isr_handler`). La ISR marca el tiempo de transición con `esp_timer_get_time()` y envía el evento a `s_input_task_handle` mediante notificaciones directas entre tareas (`vTaskNotifyGiveFromISR`). <!-- src: /firmware/main/input/keypad.c -->
- **Máquina de Estados de Antirrebote Anticipado (`debounce_step`)**:
  - `DEBOUNCE_SOURCE_EDGE`: Cuando llega un flanco, si el nivel difiere de `accepted_pressed` y la marca de tiempo actual supera `lockout_end_us`, el nuevo estado se acepta inmediatamente (`DEBOUNCE_ACTION_PRESS` o `RELEASE`). El fin del bloqueo se avanza: `lockout_end_us = now_us + lockout_us`. <!-- src: /firmware/main/input/debounce.c#debounce_step -->
  - `DEBOUNCE_SOURCE_SAMPLE`: Al expirar el bloqueo, se remuestrea el pin. Si el nivel físico coincide con el estado aceptado, el antirrebote se estabiliza; si el contacto rebotó de vuelta, se programa la liberación.

## Hall Effect Pipeline (he_backend.c & he_rt.c)

Para interruptores magnéticos, la entrada se gestiona mediante muestreo continuo por ADC en lugar de flancos GPIO: <!-- src: /firmware/main/input/he_backend.c -->

- **Muestreo Continuo por DMA**: El hardware ADC1 muestrea Key 1 (CH9) y Key 2 (CH6) en búferes DMA ping-pong continuos a `HE_SAMPLE_HZ` (250 µs por trama de conversión). <!-- src: /firmware/main/input/he_backend.c -->
- **Filtrado de Mediana (`he_frame.h`)**: La interrupción de trama en IRAM calcula la mediana de todas las conversiones válidas en la ventana de 250 µs para filtrar el ruido electromagnético sin añadir retraso de grupo.
- **Paso de Rapid Trigger (`he_rt_step`)**:
  - Compara el flujo mediano con `p->actuation` y con los umbrales dinámicos `p->rt_release` / `p->rt_press`. <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->
  - Emite `HE_RT_DOWN` o `HE_RT_UP` directamente dentro de la interrupción.
  - Solo si ocurre una transición de estado de tecla, la interrupción notifica a `he_task` para encolar el informe USB HID.

## Unified Key Event Dispatch (key_event.c)

Tanto el cauce mecánico como el de Hall Effect convergen en el despachador unificado de eventos de teclas (`firmware/main/input/key_event.c`): <!-- src: /firmware/main/input/key_event.c -->

- **Contadores de Pulsaciones Atómicos**: Incrementa `s_lifetime_presses[key_id]` y el contador específico del mapa `s_map_presses[key_id]`.
- **Telemetría de Latencia**: Llama a `latency_stats_record()` con el delta de microsegundos transcurrido entre la detección del contacto físico y el encolado en el búfer HID. <!-- src: /firmware/main/input/key_event.c -->
- **Disparo de Informes USB**: Despacha el estado a `usb_hid_set_key1_pressed()` o `usb_hid_set_key2_pressed()`, indicando a TinyUSB que transmita en el Endpoint IN 1 o Endpoint IN 2. <!-- src: /firmware/main/usb/usb_hid.c -->
- **Registro de Diagnóstico**: Añade transiciones con marca de tiempo al registro de depuración circular (`press_log_record`).
