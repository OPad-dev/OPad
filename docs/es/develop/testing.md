---
title: Pruebas y Garantía de Calidad
description: Ejecución de suites de prueba, make check, clippy y formateo de Rust, pruebas unitarias del firmware en el anfitrión y la lista de comprobación manual de hardware.
---

OPad emplea una estrategia de pruebas multinivel que abarca pruebas automatizadas de escritorio, pruebas unitarias simuladas del firmware en el anfitrión, canales de validación de CI y listas de comprobación de verificación manual de hardware. <!-- src: /AGENTS.md -->

## The Primary Verification Command (make check)

`make check` es el mecanismo de control local ejecutado por los desarrolladores antes de confirmar cambios: <!-- src: /Makefile -->

```bash
make check
```

Bajo el capó, `make check` ejecuta: <!-- src: /Makefile -->
1. **Formateo de Rust**: `cargo fmt --check` en todo el espacio de trabajo.
2. **Linter Clippy**: `cargo clippy --all-targets -D warnings` obligando a cero advertencias del compilador.
3. **Pruebas del Espacio de Trabajo**: `cargo test --workspace` ejecutando pruebas unitarias para todas las crates de escritorio (`opad-model`, `opad-protocol`, `opad-storage`, `opad-ipc`, `opad-tosu`, `opad-device`, `opad-update`, `daemon`, `gui`, `cli`).
4. **Pruebas Unitarias del Firmware en el Anfitrión**: `firmware/test/host/run_tests.sh` ejecutando pruebas unitarias en C sobre la máquina anfitriona.

## Firmware Host Unit Tests (run_tests.sh)

Dado que el hardware embebido no siempre está conectado durante el desarrollo, la lógica crítica del firmware se compila y ejecuta directamente en la máquina anfitriona utilizando `gcc` estándar: <!-- src: /firmware/test/host/run_tests.sh -->

```bash
./firmware/test/host/run_tests.sh
```

Esta suite ejecuta 28 suites de prueba independientes que cubren:
- **`test_debounce`**: Tiempos de bloqueo anticipado, supresión de rebotes y transiciones de estado de remuestreo.
- **`test_counters`**: Seguimiento monotónico de pulsaciones, avance de generaciones y reglas de preservación de deltas.
- **`test_config`**: Validación de límites de rango para ventanas de antirrebote, pines GPIO y niveles de brillo.
- **`test_frame_parser`**: Detección de marcas mágicas de tramas marcadas (`0xAA, 0x55`), validación CRC de tramas verificadas (`0xAA, 0x5A`) y recuperación tras flujos de bytes corruptos.
- **`test_he_rt`**: Cruce de umbrales de Rapid Trigger, sensibilidad dinámica de reinicio y transiciones de estado de fallo.

## Continuous Integration (CI) Checks

Además de `make check`, la integración continua de GitHub Actions valida: <!-- src: /AGENTS.md -->
- **Compilaciones Multiplataforma**: Compilaciones con MSVC de Windows para el daemon de escritorio, CLI y GUI.
- **Comprobación de Generación de Protobuf**: Ejecuta `scripts/gen_proto.sh` y falla si hay diferencias sin confirmar en `firmware/main/protocol/opad.pb.c/.h`.
- **Compilación del Firmware**: Compila el firmware para ESP32-S3 dentro del contenedor Docker `espressif/idf:v5.5.2`.
- **Cumplimiento de Licencias**: Ejecuta la comprobación de licencias REUSE (`scripts/check_reuse.py`), `cargo-deny` y auditorías de avisos de terceros.

## Manual Hardware Verification Checklist

Los cambios que tocan `firmware/main/input/`, `firmware/main/usb/`, controladores táctiles o el renderizado en pantalla requieren pruebas en hardware físico: <!-- src: /AGENTS.md -->

- Consulta `docs/es/testing-checklist.md` para ver la matriz normativa de QA de hardware. <!-- src: /docs/testing-checklist.md -->
- Las filas de prueba cubren la enumeración USB HID a 1000 Hz, actuación de interruptores mecánicos, sensibilidad de Rapid Trigger en interruptores magnéticos, detección de inmovilidad de Quick Retry táctil y recuperación por arranque de prueba en OTA.
- Nunca afirmes que una prueba de hardware ha pasado sin haberla ejecutado en un pad real (AGENTS.md §2).
