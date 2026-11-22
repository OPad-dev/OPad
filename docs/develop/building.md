---
title: Building from Source
description: Comprehensive build guide for OPad firmware using ESP-IDF v5.5.2 and desktop applications using Rust on Linux and Windows.
category: how-to
source_files:
  - /README.md
  - /Makefile
  - /firmware/CMakeLists.txt
  - /desktop/Cargo.toml
  - /AGENTS.md
---

This guide provides instructions for compiling the OPad ESP32-S3 firmware and the Rust desktop software suite (`opad-daemon`, `opad-gui`, and `opadctl`) from source. <!-- src: /Makefile -->

## Prerequisites

### 1. General Toolchain
- **Git** with submodules.
- **Python 3.10+** with `venv` support.
- **GNU Make** and standard build tools (`gcc`, `g++`, `pkg-config`).

### 2. Desktop Suite Requirements (Rust)
- **Rust Toolchain**: Stable Rust (edition 2021) installed via [rustup](https://rustup.rs). <!-- src: /desktop/Cargo.toml -->
- **Linux System Libraries**:
  - `libudev-dev`
  - `libfontconfig1-dev`
  - `libgl1-mesa-dev`
- **Windows Tools**:
  - MSVC C++ Build Tools (Visual Studio 2022).
  - Inno Setup 6 (optional, for compiling the Windows installer).

### 3. Firmware Requirements (ESP-IDF)
- **ESP-IDF v5.5.2**: OPad firmware is pinned strictly to ESP-IDF **v5.5.2** (AGENTS.md §4, §5). Building with earlier or later major releases is unsupported. <!-- src: /AGENTS.md -->

## Building the Desktop Suite

The desktop workspace is structured as a single Rust Cargo workspace (`desktop/Cargo.toml`): <!-- src: /desktop/Cargo.toml -->

```bash
cd desktop
cargo build --release
```

Binaries are generated under `desktop/target/release/`:
- `opad-daemon`: The background hardware and tosu coordinator.
- `opad-gui`: The graphical configuration and screen designer interface.
- `opadctl`: The command-line management and flashing utility.

### Running Workspace Checks
To run format checks, compiler warnings (`clippy -D warnings`), and unit tests: <!-- src: /AGENTS.md -->
```bash
make check
```

## Building the Firmware

1. **Activate ESP-IDF v5.5.2**:
   ```bash
   . $HOME/esp/esp-idf/export.sh
   ```
2. **Navigate to Firmware Directory**:
   ```bash
   cd firmware
   ```
3. **Set Target & Build**:
   ```bash
   idf.py set-target esp32s3
   idf.py build
   ```

### sdkconfig Rules
> [!IMPORTANT]
> If you edit `firmware/sdkconfig.defaults`, always delete `firmware/sdkconfig` before rebuilding (AGENTS.md §5). `idf.py fullclean` preserves the existing `sdkconfig`, causing changes to be silently ignored.

The compilation produces the following binaries in `firmware/build/`:
- `bootloader/bootloader.bin`: ESP32-S3 second-stage bootloader.
- `partition_table/partition-table.bin`: Dual OTA partition table.
- `opad-firmware.bin`: The main application binary.

## Flashing Built Firmware

To flash your freshly compiled firmware to a connected pad via `opadctl`: <!-- src: /AGENTS.md -->
```bash
opadctl flash firmware/build/opad-firmware.bin
```
This safely writes the image to the inactive OTA partition and executes a trial boot.
