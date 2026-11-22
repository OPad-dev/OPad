---
title: Subsistema de Diagnósticos del Firmware
description: Referencia de implementación para la telemetría del firmware, registro en búfer circular en memoria, autotests eléctricos de placa y análisis en banco para Hall Effect.
---

El subsistema de diagnósticos proporciona rutinas de autotest, verificación de continuidad eléctrica, registro de eventos en búfer circular en memoria y herramientas de evaluación en banco para sensores magnéticos. <!-- src: /firmware/main/diag/diag.c -->

## Diagnostic Ring Buffer Logging (diag.c)

Los eventos del firmware (conexión/desconexión USB, pérdidas de trama, discrepancias CRC, volcados a NVS) se registran en un búfer circular en memoria: <!-- src: /firmware/main/diag/diag.c -->

- **Estructura de Datos (`diag_entry_t`)**: Almacenada en un array circular de tamaño fijo `s_entries[DIAG_RING_SIZE]`. <!-- src: /firmware/main/diag/diag.c -->
- **Secciones Críticas sin Asignación de Memoria**: Los eventos se añaden mediante `diag_record(event_id, severity, arg1, arg2)`. La función adquiere un spinlock de FreeRTOS (`taskENTER_CRITICAL`), escribe la entrada y sale inmediatamente. No hay asignación dinámica de memoria ni generación de cadenas con formato en el objetivo. <!-- src: /firmware/main/diag/diag.c#diag_init -->
- **Interfaz de Drenaje**: El anfitrión drena las entradas registradas periódicamente por serie mediante envoltorios protobuf `LogEventBatch`.

## Board Electrical Self-Test (board_test.c)

Cuando se activa mediante `run_board_test` por serie o `opadctl board-test`, el firmware ejecuta comprobaciones de continuidad de hardware y periféricos: <!-- src: /firmware/main/diag/board_test.c -->

1. **Prueba de Pull en Pines GPIO Libres**: Ejercita las líneas libres de cabecera del módulo (`BOARD_MODULE_IO6_GPIO`, `IO4`, `IO2`). Alterna estados de pull-up y pull-down (`read_with_pull`), verificando que las líneas sigan los pulls internos sin cortocircuitos a tierra o a 3.3V. <!-- src: /firmware/main/diag/board_test.c#read_with_pull -->
2. **Escaneo de Bus I2C**: Escanea la dirección `0x15` (controlador táctil CST816S) y comprueba su respuesta.
3. **Comunicación con Pantalla**: Lee el registro de ID del controlador de pantalla ST7789.
4. **Identificación del Módulo de Entrada**: Muestrea el divisor analógico en GPIO 8 (`module_id`) para verificar si está instalado un módulo mecánico MX, un módulo Hall Effect V1 o un carrier V3.
5. **Generación de Informes**: Rellena `BoardTestResult` con indicadores de estado de aprobado/fallo por subsistema y mensajes de diagnóstico.

## Hall Effect Bench Test Mode (hall_bench.c)

Para la caracterización de las curvas de recorrido de interruptores magnéticos durante la validación de hardware (`CONFIG_OPAD_HALL_BENCH`): <!-- src: /firmware/main/diag/hall_bench.c -->

- **Transmisión Continua**: Transmite conversiones ADC en bruto de 12 bits directamente sobre USB CDC con resolución de milisegundos:
  `H,<ms>,<key1_raw>,<key2_raw>`
- **Agregación Estadística**: Cada segundo, calcula estadísticas de dispersión sobre miles de conversiones:
  `S,<key>,<sample_count>,<min>,<max>,<mean>,<stddev_x100>,<overflows>`
- **Harness en Python**: El script acompañante en el anfitrión `scripts/hall_bench.py` registra los flujos de datos para calcular la desviación típica, los niveles de ruido en reposo y los deltas de recorrido magnético.
