---
title: Compilación desde el Código Fuente
description: Guía completa de compilación para el firmware de OPad utilizando ESP-IDF v5.5.2 y las aplicaciones de escritorio en Rust para Linux y Windows.
---

Esta guía proporciona instrucciones para compilar desde el código fuente el firmware del ESP32-S3 de OPad y la suite de software de escritorio en Rust (`opad-daemon`, `opad-gui` y `opadctl`). <!-- src: /Makefile -->

## Prerequisites

### 1. General Toolchain
- **Git** con submódulos.
- **Python 3.10+** con soporte de `venv`.
- **GNU Make** y herramientas estándar de compilación (`gcc`, `g++`, `pkg-config`).

### 2. Desktop Suite Requirements (Rust)
- **Cadena de Herramientas Rust**: Rust estable (edición 2021) instalado a través de [rustup](https://rustup.rs). <!-- src: /desktop/Cargo.toml -->
- **Bibliotecas del Sistema en Linux**:
  - `libudev-dev`
  - `libfontconfig1-dev`
  - `libgl1-mesa-dev`
- **Herramientas en Windows**:
  - Herramientas de Compilación de MSVC C++ (Visual Studio 2022).
  - Inno Setup 6 (opcional, para compilar el instalador de Windows).

### 3. Firmware Requirements (ESP-IDF)
- **ESP-IDF v5.5.2**: El firmware de OPad está fijado estrictamente a ESP-IDF **v5.5.2** (AGENTS.md §4, §5). La compilación con versiones principales anteriores o posteriores no está soportada. <!-- src: /AGENTS.md -->

## Building the Desktop Suite

El espacio de trabajo de escritorio está estructurado como un único workspace de Cargo en Rust (`desktop/Cargo.toml`): <!-- src: /desktop/Cargo.toml -->

```bash
cd desktop
cargo build --release
```

Los binarios se generan en `desktop/target/release/`:
- `opad-daemon`: El coordinador en segundo plano del hardware y de tosu.
- `opad-gui`: La interfaz gráfica de configuración y diseñador de pantalla.
- `opadctl`: La utilidad de línea de comandos para gestión y flasheo.

### Running Workspace Checks
Para ejecutar comprobaciones de formato, advertencias del compilador (`clippy -D warnings`) y pruebas unitarias: <!-- src: /AGENTS.md -->
```bash
make check
```

## Building the Firmware

1. **Activa ESP-IDF v5.5.2**:
   ```bash
   . $HOME/esp/esp-idf/export.sh
   ```
2. **Navega al Directorio del Firmware**:
   ```bash
   cd firmware
   ```
3. **Establece el Objetivo y Compila**:
   ```bash
   idf.py set-target esp32s3
   idf.py build
   ```

### sdkconfig Rules
> [!IMPORTANT]
> Si editas `firmware/sdkconfig.defaults`, elimina siempre `firmware/sdkconfig` antes de volver a compilar (AGENTS.md §5). `idf.py fullclean` preserva el `sdkconfig` existente, haciendo que los cambios se ignoren silenciosamente.

La compilación produce los siguientes binarios en `firmware/build/`:
- `bootloader/bootloader.bin`: Bootloader de segunda etapa para ESP32-S3.
- `partition_table/partition-table.bin`: Tabla de particiones OTA dual.
- `opad-firmware.bin`: El binario principal de la aplicación.

## Flashing Built Firmware

Para flashear el firmware recién compilado en un pad conectado mediante `opadctl`: <!-- src: /AGENTS.md -->
```bash
opadctl flash firmware/build/opad-firmware.bin
```
Esto escribe la imagen de forma segura en la partición OTA inactiva y ejecuta un arranque de prueba.
