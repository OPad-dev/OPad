---
title: Compilación desde el código fuente
description: Cómo compilar la suite de escritorio OPad en Rust y el firmware ESP32-S3 con ESP-IDF v5.5.2 desde el código fuente en Linux y Windows.
sidebar:
  order: 2
---

Esta guía cubre la compilación del espacio de trabajo de escritorio en Rust ([/desktop/Cargo.toml](/desktop/Cargo.toml)) y del firmware ESP32-S3 ([/firmware/CMakeLists.txt](/firmware/CMakeLists.txt)) desde el código fuente.

## Requisitos previos

### Suite de escritorio (Rust)
- **Rust**: Cadena de herramientas estable de Rust (edición 2021) mediante [rustup](https://rustup.rs) ([/desktop/Cargo.toml](/desktop/Cargo.toml)).
- **Linux (Ubuntu / Debian)** ([/README.md](/README.md)):
  ```bash
  sudo apt update
  sudo apt install -y build-essential pkg-config libasound2-dev libudev-dev \
      libx11-dev libxkbcommon-dev libwayland-dev libfontconfig1-dev libdbus-1-dev
  ```
- **Windows**: Visual Studio MSVC C++ Build Tools; Inno Setup 6 es opcional (solo necesario para generar `opad-setup.exe` desde [/packaging/windows/installer.iss](/packaging/windows/installer.iss)).

### Firmware (ESP-IDF v5.5.2)
- **ESP-IDF v5.5.2** (versión fijada en [/AGENTS.md](/AGENTS.md) §4 y [/.github/workflows/ci.yml](/.github/workflows/ci.yml)).

## Compilación de la suite de escritorio

```bash
cd desktop
cargo build --release
```
O desde la raíz del repositorio usando [/Makefile](/Makefile):
```bash
make all
```
Los binarios de lanzamiento se generan en `desktop/target/release/`:
- `opad-daemon`
- `opad-gui`
- `opadctl`

Objetivos auxiliares opcionales en [/Makefile](/Makefile):
- `make espflash` descarga `espflash` **4.6.0** fijado y verificado por SHA-256.
- `make tosu` compila `tosu` **v4.26.2** fijado (compilación portable para glibc 2.28 por defecto; define `TOSU_PORTABLE=0` o `TOSU_STANDALONE=0` para personalizarlo).
- `make check` ejecuta `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo test --workspace` y `firmware/test/host/run_tests.sh`.

## Compilación del firmware

1. Carga el entorno de ESP-IDF v5.5.2:
   ```bash
   . $HOME/esp/esp-idf/export.sh
   ```
2. Configura el objetivo y compila:
   ```bash
   cd firmware
   idf.py set-target esp32s3
   idf.py build
   ```

> [!IMPORTANT]
> Tras editar [/firmware/sdkconfig.defaults](/firmware/sdkconfig.defaults), elimina `firmware/sdkconfig` antes de recompilar ([/AGENTS.md](/AGENTS.md) §5). `idf.py fullclean` conserva el `sdkconfig` antiguo, por lo que las ediciones se ignoran silenciosamente.

Salidas clave en `firmware/build/`:
- `bootloader/bootloader.bin`
- `partition_table/partition-table.bin`
- `ota_data_initial.bin`
- `opad-firmware.bin`

## Flasheo del firmware compilado

Actualiza la ranura OTA inactiva con protección automática de reversión:
```bash
opadctl flash firmware/build/opad-firmware.bin
```
Para un flasheo completo (bootloader, tabla de particiones, datos OTA y aplicación):
```bash
opadctl flash --full firmware/build
```
Nunca hagas commit de `build/`, `target/`, `dist/`, `firmware/sdkconfig`, `firmware/managed_components/` ni `.tools/` ([/AGENTS.md](/AGENTS.md) §5).
