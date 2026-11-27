---
title: Building from Source
description: How to build the OPad desktop suite (Rust) and the ESP32-S3 firmware (ESP-IDF v5.5.2) on Linux and Windows, including the Screen Designer preview and the pinned helper tools.
sidebar:
  order: 2
---

Two independent builds: the Rust workspace in [`desktop/`](https://github.com/OPad-dev/OPad/blob/main/desktop/Cargo.toml) and the ESP-IDF project in [`firmware/`](https://github.com/OPad-dev/OPad/blob/main/firmware/CMakeLists.txt). Neither needs the other, with one exception: the GUI's Screen Designer preview compiles the firmware's LVGL (see [below](#the-screen-designer-preview)).

## Desktop suite

### Prerequisites

- **Rust**: the stable toolchain from [rustup](https://rustup.rs) (the workspace uses edition 2021). CI installs `rustfmt` and `clippy` too.
- **Linux (Debian/Ubuntu)**: the packages CI installs ([ci.yml](https://github.com/OPad-dev/OPad/blob/main/.github/workflows/ci.yml)):
  ```bash
  sudo apt install -y build-essential pkg-config libasound2-dev libudev-dev \
      libx11-dev libxkbcommon-dev libwayland-dev libfontconfig1-dev libdbus-1-dev
  ```
- **Windows**: the MSVC toolchain (Visual Studio Build Tools with the C++ workload). Inno Setup 6 only if you build the installer ([installer.iss](https://github.com/OPad-dev/OPad/blob/main/packaging/windows/installer.iss)).

### Build

```bash
cd desktop
cargo build --release
```

or, from the repository root, `make all` ([Makefile](https://github.com/OPad-dev/OPad/blob/main/Makefile)), which runs `cargo build --release --bins` on the workspace. The binaries land in `desktop/target/release/`: `opad-daemon`, `opad-gui` and `opadctl`, plus the release tools `opad-update-helper` (the root helper the Linux packages install) and `opad-feed` (builds and verifies update feeds).

### Installing a source build

To install a build for your user on Linux, `make install-user` copies the binaries to `~/.local/bin`, adds the systemd user unit and autostart entry, and writes the `user` install-origin marker; `make install` does the same system-wide under `PREFIX` (default `/usr/local`) with the `source` marker. [Packaging](packaging.md) explains what the marker changes.

On Arch Linux, the `PKGBUILD` in [`packaging/linux/arch/`](https://github.com/OPad-dev/OPad/blob/main/packaging/linux/arch/PKGBUILD) builds an Arch package named `opad` from the checkout. <!-- src: /packaging/linux/arch/PKGBUILD -->

Two things differ from the release packages:

- The source `PKGBUILD` runs tosu with the system `nodejs`, which cannot carry `cap_sys_ptrace`; with `ptrace_scope` above 0 you have to lower it ([Troubleshooting §2](../troubleshooting.md#2-linux-tosu-cannot-read-osu-yama-ptrace_scope)). <!-- src: /packaging/linux/arch/PKGBUILD -->
- `make install-user` does not install the udev rule (`make install` does). Install it once with: <!-- src: /Makefile#install-user --> <!-- src: /Makefile#install -->

  ```bash
  sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
  sudo udevadm control --reload-rules && sudo udevadm trigger
  ```

### The Screen Designer preview

The GUI draws layout previews with the firmware's own LVGL and UI code, compiled for the host by [opad-ui-preview](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ui-preview/build.rs) and configured from the firmware's `sdkconfig`, so the preview renders exactly like the pad. It looks for LVGL in `firmware/managed_components/lvgl__lvgl` (present after one firmware build) or in `OPAD_LVGL_DIR`. Without either, it builds a stub and prints a cargo warning: everything works except the preview.

Without ESP-IDF installed:

```bash
make lvgl                                    # clones the version firmware/dependencies.lock pins
OPAD_LVGL_DIR="$PWD/build/lvgl" cargo build --release --manifest-path desktop/Cargo.toml
```

Release builds set `OPAD_REQUIRE_UI_PREVIEW=1`, which turns the missing-LVGL case into a build error.

## Firmware

### Prerequisites

[ESP-IDF **v5.5.2**](https://docs.espressif.com/projects/esp-idf/en/v5.5.2/esp32s3/get-started/index.html), the version CI builds with (`espressif/idf:v5.5.2` in [jobs.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/jobs.sh)). Other versions are not tested.

### Build

```bash
. ~/esp/esp-idf/export.sh        # wherever your ESP-IDF v5.5.2 lives
cd firmware
idf.py set-target esp32s3 build
```

`make firmware` from the root does the same build if `idf.py` is on your `PATH`, and skips it otherwise. The first build downloads the managed components (LVGL, TinyUSB and others, pinned in [dependencies.lock](https://github.com/OPad-dev/OPad/blob/main/firmware/dependencies.lock)) into `firmware/managed_components/`.

The outputs in `firmware/build/`:

| File | Flashed at | Used for |
|---|---|---|
| `opad-firmware.bin` | the inactive OTA slot | Normal updates |
| `bootloader/bootloader.bin` | `0x0` | Full flash and recovery only |
| `partition_table/partition-table.bin` | `0x8000` | Full flash and recovery only |
| `ota_data_initial.bin` | `0xf000` | Full flash and recovery only |

> [!IMPORTANT]
> After editing [sdkconfig.defaults](https://github.com/OPad-dev/OPad/blob/main/firmware/sdkconfig.defaults), delete `firmware/sdkconfig` and rebuild. `idf.py fullclean` keeps the saved config, so the edit silently does nothing ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §5).

Benchmark and debug variants are Kconfig options in [Kconfig.projbuild](https://github.com/OPad-dev/OPad/blob/main/firmware/main/Kconfig.projbuild) (for example `CONFIG_OPAD_BENCH_HID_ONLY` for [latency Stage A](latency-testing.md)), set with `idf.py menuconfig`; the `sdkconfig.*` files next to `sdkconfig.defaults` are the overlays for the diagnostic builds.

### Flash it

The daemon holds the pad's serial port; `opadctl pad flash` asks it to let go, so there is no need to stop it.

```bash
opadctl pad flash firmware/build/opad-firmware.bin    # other OTA slot, rolls back if USB never comes up
opadctl pad flash --full firmware/build               # bootloader, partition table, otadata and app
opadctl pad recover firmware/build                    # a pad that does not show up as OPad; then replug it
```

`opadctl` drives `espflash` **4.6.0**. `make espflash` downloads that exact version and checks its SHA-256 (Linux x86_64); the packages bundle it. How the flash works, and why it cannot strand a pad, is in [Updates and Automatic Rollback](../explanation/updates-and-rollback.md); the step-by-step procedures are in [Updating Firmware](../guides/update-firmware.md) and [Recovery](../recovery.md).

> [!WARNING]
> Flash on bare metal. With the pad passed through to a VM, the USB ID changes mid-flash (app `303a:8415`, ROM loader `303a:1001`) and the flash fails.

## Bundled tosu

`make tosu` builds the pinned tosu **v4.26.2** from source into `build/tosu/` (needs `pnpm` and Node 24). By default (`TOSU_STANDALONE=1`, `TOSU_PORTABLE=1`) it is a self-contained binary whose native parts are rebuilt with zig and docker for glibc 2.28, so it runs on older distributions; `TOSU_STANDALONE=0` instead produces a `dist/` that runs on the system `node`, as the Arch package does. You only need it to build packages or to test the bundled copy; a source build of the app can use an installed tosu.

## Never commit

`build/`, `target/`, `dist/`, `firmware/sdkconfig`, `firmware/managed_components/`, or anything in `.tools/` ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §5).
