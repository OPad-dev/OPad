---
title: Pruebas
description: Las comprobaciones que tiene que superar un cambio - make check, las 28 pruebas de anfitrión del firmware, los trabajos de CI y dónde se ejecutan (el laboratorio antes de cada push de main, GitHub en los pull requests y las publicaciones), y la lista de comprobación en el teclado.
sidebar:
  order: 3
---

Un cambio se prueba en cuatro niveles: `make check` en tu máquina, los trabajos completos de CI, las instalaciones del laboratorio al publicar y, para todo lo que toque la entrada, el USB, el tacto o la pantalla, un teclado real ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §4). Si algo no se pudo probar (sin teclado conectado, sin máquina Windows), dilo en el commit o en el pull request.

## `make check`

```bash
make check
```

Ejecútalo antes de dar cualquier cambio por terminado. En el [Makefile](https://github.com/OPad-dev/OPad/blob/main/Makefile) ejecuta, por orden:

1. `cargo fmt --all -- --check` sobre el workspace `desktop/`;
2. `cargo clippy --all-targets -- -D warnings` (los avisos son errores);
3. `cargo test` sobre todo el workspace (pruebas unitarias y de integración de cada crate, el daemon, la GUI y la CLI);
4. [`firmware/test/host/run_tests.sh`](https://github.com/OPad-dev/OPad/blob/main/firmware/test/host/run_tests.sh), las pruebas de anfitrión del firmware;
5. [`scripts/release/check_versions.py`](https://github.com/OPad-dev/OPad/blob/main/scripts/release/check_versions.py), que comprueba que coinciden las versiones de `desktop/Cargo.toml`, `firmware/CMakeLists.txt`, `.release-please-manifest.json` y `Cargo.lock`.

No compila el firmware, no regenera el protobuf ni ejecuta las comprobaciones de licencias; eso lo hace CI (abajo).

## Pruebas de anfitrión del firmware

La lógica del firmware que no depende del hardware es C puro, compilado con el `gcc -Wall -Wextra -Werror` del anfitrión y ejecutado directamente, así que se puede probar sin teclado ni ESP-IDF. `run_tests.sh` compila y ejecuta **28** programas de prueba (`test_config` dos veces, una con `-DCONFIG_OPAD_BENCH_DEBUG_GPIO=1`):

| Área | Pruebas |
|---|---|
| Teclas y módulos | `test_debounce`, `test_press_log`, `test_keypad_staged`, `test_game_keys`, `test_module_id`, `test_module_watch` |
| Hall Effect y ADS7028 | `test_he_rt`, `test_he_frame`, `test_he_health`, `test_he_cal`, `test_he_cal_capture`, `test_he_cal_button`, `test_he_cal_runner`, `test_he_cal_sampling`, `test_he_cal_storage`, `test_he_cal_touch_guard`, `test_he_cal_barrier`, `test_ads7028`, `test_he_spi_frame` |
| Tacto y USB | `test_gesture`, `test_swipe_labels`, `test_usb_report_barrier` |
| Almacenamiento, configuración, protocolo, diagnósticos | `test_counters`, `test_config`, `test_owner`, `test_nvs_ns`, `test_frame_parser`, `test_diag` |

Algunas pruebas enlazan pequeños stubs de las API de ESP-IDF (`cal_stubs/`, `barrier_stubs/`, `nvs_stubs/` en [firmware/test/host](https://github.com/OPad-dev/OPad/tree/main/firmware/test/host)). La lógica nueva del firmware que se pueda escribir sin llamadas al hardware va en un archivo de C puro con su prueba aquí; una prueba nueva se añade a `run_tests.sh` y a sus contadores `[n/28]`.

## Trabajos de CI

Las comprobaciones se escriben una sola vez, como funciones de shell en [scripts/ci/jobs.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/jobs.sh) (Linux) y [scripts/ci/jobs.ps1](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/jobs.ps1) (Windows). Tanto el laboratorio como GitHub ejecutan estos scripts, así que una comprobación pasa o falla igual en ambos sitios. Un cambio en una comprobación va en los scripts, nunca solo en `ci.yml`.

| Trabajo | Ejecuta |
|---|---|
| `versions` | `check_versions.py` |
| `rust` | `cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test --workspace` |
| `firmware-host-tests` | `run_tests.sh` |
| `proto-check` | `scripts/gen_proto.sh` con nanopb 0.4.9.1, y luego `git diff --exit-code firmware/main/protocol/` |
| `firmware-build` | `idf.py set-target esp32s3 build` dentro de `espressif/idf:v5.5.2` |
| `licences` | `scripts/check_reuse.py` (REUSE), `cargo deny check` y la generación de los avisos de terceros (`make notices`) |
| `rust-windows` | fmt, clippy, compilación y pruebas en MSVC, todos los crates salvo `opad-gui` |
| `rust-windows-gui` | clippy, compilación y pruebas de `opad-gui` en MSVC, aparte para que el port a Windows de un crate no oculte los demás |

Puedes ejecutar cualquiera en local: `scripts/ci/jobs.sh list`, `scripts/ci/jobs.sh proto-check` o `scripts/ci/jobs.sh all`. En una máquina sin `/opt/esp/idf`, `firmware-build` se ejecuta a sí mismo dentro de la imagen docker `espressif/idf:v5.5.2`.

### Dónde se ejecuta CI

- **El laboratorio, antes de cada push de `main`.** [scripts/ci/lab-ci.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/lab-ci.sh) empaqueta exactamente un commit, lo copia al mini PC del laboratorio y ejecuta en paralelo los trabajos de Linux en su VM `ci-linux` y los de Windows en el propio mini PC (con el checkout CRLF de Git for Windows, como en los runners de GitHub). Cuando pasan todos los trabajos, registra el commit. `make lab-ci` lo ejecuta para `HEAD`; `make push` lo ejecuta para `main` y después hace push. El hook pre-push ([scripts/git-hooks/pre-push](https://github.com/OPad-dev/OPad/blob/main/scripts/git-hooks/pre-push), instalado con `make hooks`) se niega a subir un `main` que el laboratorio no haya aprobado. Solo se comprueba `main`; las demás ramas se suben libremente. Un commit que ya pasó no se vuelve a ejecutar salvo que se indique `--force`. El laboratorio en sí se describe en [Laboratorio de pruebas](lab.md).
- **GitHub Actions** ([ci.yml](https://github.com/OPad-dev/OPad/blob/main/.github/workflows/ci.yml)), que **no** se ejecuta en los push: se ejecuta en los pull requests, a mano (`gh workflow run ci.yml --ref <branch>`) y antes de cada publicación, cuando [release-please.yml](https://github.com/OPad-dev/OPad/blob/main/.github/workflows/release-please.yml) lo llama tras fusionarse un PR de publicación; la versión solo se construye si pasa.

## En un teclado

Las pruebas de anfitrión no ven los tiempos, la enumeración USB ni un panel táctil real. Los cambios en `firmware/main/input/`, `firmware/main/usb/`, el tacto o la pantalla se prueban además en un teclado real, siguiendo las filas de [la lista de comprobación de pruebas](../testing-checklist.md). Un cambio en la ruta de pulsación indica también su efecto en la latencia; [Medición de latencia](latency-testing.md) describe cómo medirlo.

Los instaladores y las actualizaciones se prueban en el laboratorio de VM como parte de cada publicación de la app ([Laboratorio de pruebas](lab.md)).
