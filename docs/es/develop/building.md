---
title: Compilar desde el código fuente
description: Cómo compilar la suite de escritorio de OPad (Rust) y el firmware del ESP32-S3 (ESP-IDF v5.5.2) en Linux y Windows, incluida la vista previa del Screen Designer y las herramientas auxiliares con versión fijada.
sidebar:
  order: 2
---

Son dos compilaciones independientes: el workspace de Rust en [`desktop/`](https://github.com/OPad-dev/OPad/blob/main/desktop/Cargo.toml) y el proyecto ESP-IDF en [`firmware/`](https://github.com/OPad-dev/OPad/blob/main/firmware/CMakeLists.txt). Ninguna necesita a la otra, con una excepción: la vista previa del Screen Designer de la GUI compila la LVGL del firmware (consulta [más abajo](#la-vista-previa-del-screen-designer)).

## Suite de escritorio

### Requisitos previos

- **Rust**: la toolchain estable de [rustup](https://rustup.rs) (el workspace usa la edición 2021). CI instala también `rustfmt` y `clippy`.
- **Linux (Debian/Ubuntu)**: los paquetes que instala CI ([ci.yml](https://github.com/OPad-dev/OPad/blob/main/.github/workflows/ci.yml)):
  ```bash
  sudo apt install -y build-essential pkg-config libasound2-dev libudev-dev \
      libx11-dev libxkbcommon-dev libwayland-dev libfontconfig1-dev libdbus-1-dev
  ```
- **Windows**: la toolchain MSVC (Visual Studio Build Tools con la carga de trabajo de C++). Inno Setup 6 solo si compilas el instalador ([installer.iss](https://github.com/OPad-dev/OPad/blob/main/packaging/windows/installer.iss)).

### Compilar

```bash
cd desktop
cargo build --release
```

o, desde la raíz del repositorio, `make all` ([Makefile](https://github.com/OPad-dev/OPad/blob/main/Makefile)), que ejecuta `cargo build --release --bins` sobre el workspace. Los binarios quedan en `desktop/target/release/`: `opad-daemon`, `opad-gui` y `opadctl`, más las herramientas de publicación `opad-update-helper` (el asistente con root que instalan los paquetes de Linux) y `opad-feed` (construye y verifica los feeds de actualización).

### Instalar una compilación desde el código fuente

Para instalar una compilación para tu usuario en Linux, `make install-user` copia los binarios a `~/.local/bin`, añade la unidad de usuario de systemd y la entrada de inicio automático, y escribe el marcador de origen de instalación `user`; `make install` hace lo mismo para todo el sistema bajo `PREFIX` (por defecto `/usr/local`) con el marcador `source`. [Empaquetado](packaging.md) explica qué cambia el marcador.

En Arch Linux, el `PKGBUILD` de [`packaging/linux/arch/`](https://github.com/OPad-dev/OPad/blob/main/packaging/linux/arch/PKGBUILD) compila desde la copia del repositorio un paquete de Arch llamado `opad`. <!-- src: /packaging/linux/arch/PKGBUILD -->

Hay dos diferencias con los paquetes de las versiones publicadas:

- El `PKGBUILD` del código fuente ejecuta tosu con el `nodejs` del sistema, que no puede llevar `cap_sys_ptrace`; con `ptrace_scope` por encima de 0 tienes que bajarlo ([Solución de problemas §2](../troubleshooting.md#2-linux-tosu-no-puede-leer-osu-yama-ptrace_scope)). <!-- src: /packaging/linux/arch/PKGBUILD -->
- `make install-user` no instala la regla udev (`make install` sí). Instálala una vez con: <!-- src: /Makefile#install-user --> <!-- src: /Makefile#install -->

  ```bash
  sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/
  sudo udevadm control --reload-rules && sudo udevadm trigger
  ```

### La vista previa del Screen Designer

La GUI dibuja las vistas previas de los diseños con la propia LVGL y el código de interfaz del firmware, compilados para el anfitrión por [opad-ui-preview](https://github.com/OPad-dev/OPad/blob/main/desktop/crates/opad-ui-preview/build.rs) y configurados a partir del `sdkconfig` del firmware, así que la vista previa se ve exactamente igual que en el teclado. Busca LVGL en `firmware/managed_components/lvgl__lvgl` (presente tras compilar el firmware una vez) o en `OPAD_LVGL_DIR`. Sin ninguna de las dos, compila un stub e imprime un aviso de cargo: todo funciona salvo la vista previa.

Sin ESP-IDF instalado:

```bash
make lvgl                                    # clones the version firmware/dependencies.lock pins
OPAD_LVGL_DIR="$PWD/build/lvgl" cargo build --release --manifest-path desktop/Cargo.toml
```

Las compilaciones de publicación activan `OPAD_REQUIRE_UI_PREVIEW=1`, que convierte la falta de LVGL en un error de compilación.

## Firmware

### Requisitos previos

[ESP-IDF **v5.5.2**](https://docs.espressif.com/projects/esp-idf/en/v5.5.2/esp32s3/get-started/index.html), la versión con la que compila CI (`espressif/idf:v5.5.2` en [jobs.sh](https://github.com/OPad-dev/OPad/blob/main/scripts/ci/jobs.sh)). Las demás versiones no se prueban.

### Compilar

```bash
. ~/esp/esp-idf/export.sh        # wherever your ESP-IDF v5.5.2 lives
cd firmware
idf.py set-target esp32s3 build
```

`make firmware` desde la raíz hace la misma compilación si `idf.py` está en tu `PATH`, y si no la omite. La primera compilación descarga los componentes gestionados (LVGL, TinyUSB y otros, fijados en [dependencies.lock](https://github.com/OPad-dev/OPad/blob/main/firmware/dependencies.lock)) en `firmware/managed_components/`.

Los resultados en `firmware/build/`:

| Archivo | Se flashea en | Se usa para |
|---|---|---|
| `opad-firmware.bin` | la ranura OTA inactiva | Actualizaciones normales |
| `bootloader/bootloader.bin` | `0x0` | Solo flasheo completo y recuperación |
| `partition_table/partition-table.bin` | `0x8000` | Solo flasheo completo y recuperación |
| `ota_data_initial.bin` | `0xf000` | Solo flasheo completo y recuperación |

> [!IMPORTANT]
> Después de editar [sdkconfig.defaults](https://github.com/OPad-dev/OPad/blob/main/firmware/sdkconfig.defaults), borra `firmware/sdkconfig` y vuelve a compilar. `idf.py fullclean` conserva la configuración guardada, así que la edición no hace nada sin avisar ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §5).

Las variantes de benchmark y de depuración son opciones de Kconfig en [Kconfig.projbuild](https://github.com/OPad-dev/OPad/blob/main/firmware/main/Kconfig.projbuild) (por ejemplo `CONFIG_OPAD_BENCH_HID_ONLY` para la [etapa A de latencia](latency-testing.md)), que se ajustan con `idf.py menuconfig`; los archivos `sdkconfig.*` junto a `sdkconfig.defaults` son las capas para las compilaciones de diagnóstico.

### Flashearlo

El daemon tiene abierto el puerto serie del teclado; `opadctl flash` le pide que lo suelte, así que no hace falta pararlo.

```bash
opadctl flash firmware/build/opad-firmware.bin    # other OTA slot, rolls back if USB never comes up
opadctl flash --full firmware/build               # bootloader, partition table, otadata and app
opadctl recover firmware/build                    # a pad that does not show up as OPad; then replug it
```

`opadctl` maneja `espflash` **4.6.0**. `make espflash` descarga esa versión exacta y comprueba su SHA-256 (Linux x86_64); los paquetes lo incluyen. Cómo funciona el flasheo, y por qué no puede dejar inservible un teclado, está en [Actualizaciones y reversión automática](../explanation/updates-and-rollback.md); los procedimientos paso a paso están en [Actualizar el firmware](../guides/update-firmware.md) y [Recuperación](../recovery.md).

> [!WARNING]
> Flashea en máquina física. Con el teclado pasado a una VM, el ID USB cambia a mitad del flasheo (app `303a:8415`, cargador de la ROM `303a:1001`) y el flasheo falla.

## tosu incluido

`make tosu` compila desde el código fuente el tosu fijado **v4.26.2** en `build/tosu/` (necesita `pnpm` y Node 24). Por defecto (`TOSU_STANDALONE=1`, `TOSU_PORTABLE=1`) es un binario autónomo cuyas partes nativas se recompilan con zig y docker para glibc 2.28, así que funciona en distribuciones más antiguas; `TOSU_STANDALONE=0` produce en su lugar un `dist/` que se ejecuta con el `node` del sistema, como hace el paquete de Arch. Solo lo necesitas para construir paquetes o para probar la copia incluida; una compilación de la app desde el código fuente puede usar un tosu instalado.

## Nunca commitear

`build/`, `target/`, `dist/`, `firmware/sdkconfig`, `firmware/managed_components/`, ni nada de `.tools/` ([AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §5).
