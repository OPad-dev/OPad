---
title: Medición de latencia
description: Cómo medir la latencia de las teclas de OPad - el histograma en el propio teclado, las etapas de benchmark A a D que aíslan el coste de cada subsistema, el filtro de publicación y los scripts que registran una prueba.
sidebar:
  order: 9
---

La latencia siempre gana ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §1.1), así que un cambio que podría ralentizar las teclas hay que medirlo, no argumentarlo. El método es medir lo mismo con cada vez más partes del firmware activadas: si la pila completa no es más lenta que la ruta de teclas sola, nada más se interpone. Por qué la ruta de teclas está construida como está se explica en [Ingeniería de latencia](../explanation/latency.md).

## Qué se mide

[latency_stats.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/input/latency_stats.c) registra, para cada cambio de tecla, el tiempo desde el flanco del interruptor (`edge_us`, tomado en la interrupción) hasta que TinyUSB acepta el informe HID:

- 500 cubetas de 10 µs (0–5 ms). `p50`, `p99` y `p99.9` son el borde superior de una cubeta, así que siempre son múltiplos de 10 µs: **no se pueden distinguir diferencias de menos de 10 µs**. `max` es exacto.
- `deferred_reports`: cambios que tuvieron que esperar a una transferencia en curso en su endpoint.
- Las muestras por encima de 1 ms se cuentan como valores atípicos y se comunican al anfitrión como diagnósticos `DIAG_EVENT_LATENCY_OUTLIER`.

El propio sondeo de 1 ms del anfitrión y todo lo que viene después no está en estas cifras; `bench_latency.py` (abajo) mira ese lado en Linux.

## Filtro de publicación

1. Ninguna pulsación perdida en streams rápidos alternados (por encima de 15–20 pulsaciones/s).
2. Ninguna tecla atascada con ninguna carga.
3. Ningún valor atípico por encima de 1 ms causado por la pantalla, el tráfico CDC, los temporizadores o las escrituras en flash.
4. El `p99.9` de la etapa D, menos de 100 µs por encima del de la etapa A, en la misma plataforma.
5. La ISR de teclas, las tareas de entrada y TinyUSB en el núcleo 0; pantalla, protocolo, tacto y ejecución en el núcleo 1.

## Etapas de benchmark

```mermaid
flowchart LR
    A["A: solo teclas + HID"] --> B["B: + protocolo CDC, daemon conectado"] --> C["C: + pantalla a 2 / 10 / 30 Hz"] --> D["D: todo"]
```

| Etapa | Compilación | Qué se ejecuta | Lectura de las estadísticas |
|---|---|---|---|
| **A** | `CONFIG_OPAD_BENCH_HID_ONLY=y` | Solo la tarea del teclado y TinyUSB HID; sin CDC, ejecución ni pantalla | Se imprimen cada 10 s solo por **UART0** (TX = GPIO43, 115200 baudios): esta compilación no tiene CDC, así que hace falta un adaptador USB-UART. Reinicia para ponerlas a cero. |
| **B** | `CONFIG_OPAD_BENCH_NO_DISPLAY=y` | Añade el protocolo CDC con el daemon conectado; pantalla y retroiluminación apagadas | `opadctl latency` |
| **C** | Compilación estándar | Añade la pantalla de partida alimentada por tosu, al límite de refresco de partida (`gameplay_display_hz`: 2, 10 (por defecto) y 30 Hz) | `opadctl latency` |
| **D** | Compilación estándar | Todo: streaming de tosu, LVGL, sincronización tras el enfriamiento y en reposo, diagnósticos | `opadctl latency` |

Las opciones de Kconfig están en [Kconfig.projbuild](https://github.com/OPad-dev/OPad/blob/main/firmware/main/Kconfig.projbuild); consulta [Compilar](building.md#firmware) para ver cómo ajustarlas. `CONFIG_OPAD_BENCH_DEBUG_GPIO` además conmuta un pin (por defecto GPIO 4, `CONFIG_OPAD_BENCH_DEBUG_GPIO_NUM`) justo después de cada envío HID, para un analizador lógico; comprueba antes que el pin está libre en tu cableado.

## Registrar una prueba

[scripts/latency_row.py](https://github.com/OPad-dev/OPad/blob/main/scripts/latency_row.py) envuelve `opadctl latency` y convierte el resultado en una fila de tabla:

```bash
python3 scripts/latency_row.py --reset                 # right before you start tapping
# tap both keys alternately, above 15 presses/s, for 2 minutes
python3 scripts/latency_row.py --stage "C (10 Hz)"     # prints the row
```

`--stage` acepta uno de `A`, `B`, `C (2 Hz)`, `C (10 Hz)`, `C (30 Hz)`, `D` (`KNOWN_STAGES`). La columna del commit sale de git y se marca con `-dirty` cuando `firmware/` o `protocol/` tienen cambios sin commitear, porque una fila que nombra un commit que no es lo que se flasheó es peor que ninguna. Las filas van en la tabla de abajo; la plataforma, en las notas. El filtro solo compara etapas de la misma plataforma: una etapa D en Windows frente a una etapa A en Linux mide la diferencia equivocada.

| Fecha | Commit del firmware | Etapa | Muestras | p50 (µs) | p99 (µs) | p99.9 (µs) | Máx. (µs) | Diferidos | Teclas atascadas / perdidas | Notas |
|---|---|---|---|---|---|---|---|---|---|---|

Todavía no se ha registrado en esta tabla ninguna prueba por etapas A–D. Las mediciones sueltas con el firmware estándar están en [la lista de comprobación de pruebas](../testing-checklist.md) (filas HW-04, HW-14, PPM-04).

## Lado del anfitrión (Linux)

[scripts/bench_latency.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/bench_latency.sh) ejecuta [bench_latency.py](https://github.com/OPad-dev/OPad/blob/main/scripts/bench_latency.py). Busca el dispositivo de entrada del teclado (`/dev/input/by-id/*OPad*`, o por nombre en `/sys/class/input/event*`), recoge 100 intervalos de menos de 50 ms entre eventos `EV_KEY` e imprime su media, mínimo, máximo y desviación típica, frente a unos objetivos de desviación típica inferior a 0,25 ms y máximo de 2,5 ms. Necesita acceso de lectura al dispositivo de eventos. No hay equivalente para Windows; en Windows, las etapas A–D se miden en el teclado, que es la parte de la que trata el filtro.
