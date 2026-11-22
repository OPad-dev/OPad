---
title: Pruebas y control de calidad
description: Cómo ejecutar make check, las pruebas de Cargo del escritorio, las 28 pruebas unitarias del firmware en anfitrión, las comprobaciones de CI y la lista manual de verificación en hardware.
sidebar:
  order: 3
---

OPad verifica los cambios en cuatro capas: `make check` local, 28 pruebas unitarias en C compiladas en anfitrión para la lógica del firmware, CI en GitHub Actions y verificación manual en hardware ([/AGENTS.md](/AGENTS.md) §4, [/Makefile](/Makefile)).

## Puerta local principal (`make check`)

Ejecuta antes de dar cualquier cambio por terminado:
```bash
make check
```
En [/Makefile](/Makefile), `make check` ejecuta:
1. `cargo fmt --check` (formato en todo el espacio de trabajo `desktop/`)
2. `cargo clippy --all-targets -- -D warnings` (advertencias prohibidas)
3. `cargo test --workspace` (pruebas unitarias y de integración en todos los crates de escritorio, daemon, GUI y CLI)
4. `./firmware/test/host/run_tests.sh` (las 28 suites de pruebas unitarias en anfitrión del firmware)

## Pruebas unitarias del firmware en anfitrión (`run_tests.sh`)

Los módulos C independientes del hardware se compilan con `gcc -Wall -Wextra -Werror` y se ejecutan directamente en el anfitrión mediante [/firmware/test/host/run_tests.sh](/firmware/test/host/run_tests.sh):

```bash
./firmware/test/host/run_tests.sh
```

El script ejecuta **28** suites de pruebas que cubren:
- **Entrada y mecánico**: `test_debounce`, `test_press_log`, `test_keypad_staged`, `test_game_keys`, `test_module_id`, `test_module_watch`
- **Hall Effect y ADS7028**: `test_he_rt`, `test_he_frame`, `test_he_cal`, `test_he_cal_capture`, `test_he_health`, `test_he_cal_button`, `test_he_cal_runner`, `test_he_cal_sampling`, `test_he_cal_storage`, `test_he_cal_touch_guard`, `test_he_cal_barrier`, `test_ads7028`, `test_he_spi_frame`
- **Táctil y USB**: `test_gesture`, `test_swipe_labels`, `test_usb_report_barrier`
- **Almacenamiento, configuración, protocolo y diagnóstico**: `test_counters`, `test_config` (más `-DCONFIG_OPAD_BENCH_DEBUG_GPIO=1`), `test_diag`, `test_frame_parser`, `test_owner`, `test_nvs_ns`

## Integración continua (CI)

GitHub Actions ([/.github/workflows/ci.yml](/.github/workflows/ci.yml)) ejecuta adicionalmente:
- Compilación y pruebas MSVC en Windows (con la GUI en un trabajo separado).
- Regeneración de protobuf mediante [/scripts/gen_proto.sh](/scripts/gen_proto.sh) (falla si los archivos generados `opad.pb.c/.h` difieren).
- Compilación del firmware ESP32-S3 en el contenedor `espressif/idf:v5.5.2`.
- Comprobaciones de licencias y dependencias: cumplimiento REUSE ([/scripts/check_reuse.py](/scripts/check_reuse.py)), `cargo-deny` y verificación de avisos de terceros.

## Lista manual de verificación en hardware

Los cambios que afecten a `firmware/main/input/`, `firmware/main/usb/`, al manejo de la pantalla táctil o al renderizado de pantalla deben probarse en un teclado real usando [la lista de verificación de pruebas](../testing-checklist.md) ([/AGENTS.md](/AGENTS.md) §2, §4). Informa siempre con claridad si no había hardware conectado y se omitieron pruebas.
