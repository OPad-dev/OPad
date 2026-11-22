---
title: Hall Effect y Rapid Trigger
description: Explicación de la detección magnética por efecto Hall, muestreo continuo por ADC, filtrado de mediana, máquinas de estado de Rapid Trigger y almacenamiento de calibración.
---

Los interruptores magnéticos de efecto Hall (HE) sustituyen las láminas de contacto metálicas físicas por un imán permanente incrustado en el vástago del interruptor y un sensor Hall lineal en la PCB del teclado. A diferencia de los interruptores mecánicos que actúan en un punto físico fijo de contacto entre láminas, los interruptores Hall Effect permiten puntos de actuación dinámicos y **Rapid Trigger (RT)**: restablecer la tecla en el instante en que comienza el movimiento ascendente del dedo, y reactivarla en el momento en que se reanuda el movimiento descendente. <!-- src: /firmware/main/input/he_backend.c -->

## Continuous ADC Sampling & Median Filtering

En el ESP32-S3, la detección por efecto Hall se implementa con extremo cuidado para mantener las garantías de latencia: <!-- src: /firmware/main/input/he_backend.c -->

- **Muestreo Continuo por DMA**: El ADC1 opera en modo DMA por hardware continuo, alternando entre Key 1 (CH9) y Key 2 (CH6) a alta frecuencia de muestreo (`HE_SAMPLE_HZ`). Cada trama DMA se completa en 250 µs (`HE_FRAME_CONVS`). <!-- src: /firmware/main/input/he_backend.c -->
- **Interrupción de Trama Fijada en IRAM**: La interrupción por finalización de trama (`on_frame`) se ejecuta en Core 0 enteramente en la RAM rápida interna (`IRAM_ATTR`). Permanece activa y determinista incluso durante operaciones de escritura en flash (`ADC_CONTINUOUS_ISR_IRAM_SAFE`). <!-- src: /firmware/main/input/he_backend.c -->
- **Filtrado de Mediana**: Para eliminar picos de ruido eléctrico sin introducir retraso de fase, la interrupción de trama calcula la mediana de las conversiones válidas dentro de cada trama DMA de 250 µs. <!-- src: /firmware/main/input/he_backend.c -->
- **Cero Sobrecarga de Sondeo**: El motor de decisión da pasos dentro de la interrupción. La tarea de FreeRTOS (`he_task`) solo se despierta cuando ocurre una transición real de estado de tecla, eliminando cambios de contexto innecesarios. <!-- src: /firmware/main/input/he_backend.c -->

## Rapid Trigger Decision Engine (he_rt.c)

La máquina de estados de Rapid Trigger rastrea el movimiento del vástago a través de cuatro estados operativos diferenciados: <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->

```
                raw >= actuation
       +--------------------------------+
       |                                |
       v                                |
+-------------+   raw <= extreme - rt_release    +------------------+
|   PRESSED   | -------------------------------> |  RT_RELEASED     |
+-------------+                                  +------------------+
       |                                                |
       | raw <= region_exit                             | raw >= extreme + rt_press
       v                                                v
+-------------+ <--------------------------------+------+
|    ARMED    |       raw <= region_exit         |
+-------------+                                  v
                                           (Re-pulsar Abajo)
```

### State Transitions
1. **`HE_RT_ARMED`**: La tecla está liberada en su zona de reposo. Cuando el movimiento hacia abajo cruza el umbral de actuación configurado (`raw >= p->actuation`), la tecla pasa a `PRESSED` y emite `HE_RT_DOWN`. La profundidad de recorrido instantánea se registra en `k->extreme`. <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->
2. **`HE_RT_PRESSED`**: La tecla se mantiene activamente presionada.
   - Si el dedo empuja más profundamente, `k->extreme` rastrea la profundidad máxima (`cap_peak`).
   - Si el dedo sube una distancia equivalente a la sensibilidad de reinicio (`raw <= k->extreme - p->rt_release`), la tecla pasa inmediatamente a `HE_RT_RT_RELEASED` y emite `HE_RT_UP`—sin necesidad de que la tecla vuelva al punto superior de reposo. <!-- src: /firmware/main/input/he_rt.c#he_rt_step -->
   - Si el recorrido regresa completamente más allá del límite superior (`region_exit`), vuelve a `ARMED`.
3. **`HE_RT_RT_RELEASED`**: La tecla se liberó a mitad de recorrido mediante Rapid Trigger.
   - Si se reanuda el movimiento hacia abajo superando la sensibilidad de pulsación (`raw >= k->extreme + p->rt_press`), la tecla se activa inmediatamente (`HE_RT_DOWN`) y regresa a `PRESSED`.
   - Si el recorrido ascendente continúa más allá de `region_exit`, la tecla regresa a `ARMED`.

## Travel Units & Calibration (Decision Record D3)

Conforme al registro de decisiones normativo D3 (`docs/specs/v2-hall-effect-decisions.md`): <!-- src: /docs/specs/v2-hall-effect-decisions.md -->
- **Porcentaje del Rango Magnético**: El recorrido de un interruptor Hall nunca se mide en milímetros. Las tolerancias de fabricación, la potencia de los imanes y las dimensiones de la carcasa hacen que las curvas de campo magnético varíen entre interruptores. Todos los puntos de actuación y distancias de Rapid Trigger se configuran estrictamente como un porcentaje del rango magnético calibrado (en unidades de 0.1%).
- **Calibración Guiada de Dos Puntos**:
  1. **Nivel de Reposo**: Mide el flujo magnético cuando el vástago del interruptor está en reposo sin presionar.
  2. **Nivel de Fondo (Bottom-Out)**: Mide el flujo magnético cuando el interruptor hace tope contra la carcasa.
- **Almacenes de Calibración Independientes**: Debido a que los módulos de hardware V1 (ADC interno) y V3 (ADC SPI) operan con diferentes ganancias de señal y líneas base de voltaje, cada tipo de módulo mantiene un espacio de nombres de calibración independiente en NVS (`s_store12`). Una calibración V1 nunca se aplica a un módulo V3. <!-- src: /firmware/main/input/he_cal.c -->

## Safety & Stall Recovery

Para garantizar la fiabilidad del interruptor y evitar entradas atascadas: <!-- src: /firmware/main/input/he_backend.c -->
- **Watchdog de Salud del ADC**: Si una tecla no recibe una conversión ADC válida dentro de `HE_STALL_US`, la tecla se libera de inmediato para evitar que quede atascada presionada.
- **Recuperación tras Fallo (`he_rt_fault`)**: Al recuperarse de un bloqueo del ADC, se debe observar que la tecla vuelva al estado superior de reposo (`need_release = true`) antes de que se pueda registrar una nueva pulsación. La recuperación del sensor nunca emitirá una pulsación accidental. <!-- src: /firmware/main/input/he_rt.c#he_rt_init -->
- **Bloqueo sin Calibración (Decisión D7)**: Un pad Hall Effect sin calibrar queda bloqueado en su asistente de calibración en pantalla. Rapid Trigger y los informes de teclado permanecen desactivados hasta que los umbrales de reposo y fondo se registren con éxito. <!-- src: /docs/specs/v2-hall-effect-decisions.md -->
