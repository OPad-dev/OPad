//! Compiles the firmware's LVGL (managed component) and UI core for the host, configured
//! from the firmware's own sdkconfig so preview and device render identically.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let firmware = strip_verbatim(
        manifest
            .join("../../../firmware")
            .canonicalize()
            .expect("firmware directory"),
    );
    let lvgl = firmware.join("managed_components/lvgl__lvgl");
    let ui_core = firmware.join("main/ui/core");
    let sdkconfig = firmware.join("sdkconfig");
    if !lvgl.exists() || !sdkconfig.exists() {
        panic!(
            "LVGL sources or sdkconfig missing. Configure the firmware once: \
             idf.py -C {} reconfigure",
            firmware.display()
        );
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let kconfig_header = out.join("lv_kconfig_host.h");
    std::fs::write(&kconfig_header, lvgl_defines(&sdkconfig)).unwrap();

    let mut build = cc::Build::new();
    build
        .include(&lvgl)
        .include(lvgl.join("src"))
        .include(&ui_core)
        // The generated header is reached by name from OUT_DIR, and the
        // macro carries angle brackets rather than a quoted absolute path.
        // `#include LV_CONF_KCONFIG_EXTERNAL_INCLUDE` needs the delimiters to
        // survive the compiler's command line, and the quotes in a
        // `-DX="C:\path"` do not on MSVC: they are stripped before the
        // preprocessor sees them, so the macro expanded bare and every LVGL
        // file died with `error C2006: '#include': expected "FILENAME"`.
        // Angle brackets need no quoting on any toolchain, and dropping the
        // absolute path sidesteps backslash-escape trouble as well.
        .include(&out)
        .define(
            "LV_CONF_KCONFIG_EXTERNAL_INCLUDE",
            Some("<lv_kconfig_host.h>"),
        )
        .flag_if_supported("-std=gnu11")
        .flag_if_supported("-w")
        .opt_level(2)
        .warnings(false);

    for file in c_files(&lvgl.join("src")) {
        build.file(file);
    }
    for name in ["ui_data.c", "ui_screen.c", "ui_defaults.c"] {
        build.file(ui_core.join(name));
    }
    build.file(manifest.join("csrc/preview.c"));
    build.compile("osupad_ui_lvgl");

    println!("cargo:rerun-if-changed={}", sdkconfig.display());
    println!("cargo:rerun-if-changed={}", ui_core.display());
    println!("cargo:rerun-if-changed=csrc/preview.c");
}

/// `CONFIG_LV_*` lines of sdkconfig as C defines (what ESP-IDF puts in sdkconfig.h)
fn lvgl_defines(sdkconfig: &Path) -> String {
    let mut header = String::from("#pragma once\n");
    for line in std::fs::read_to_string(sdkconfig).unwrap().lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if !key.starts_with("CONFIG_LV_") && !key.starts_with("CONFIG_LVGL_") {
            continue;
        }
        let value = if value == "y" { "1" } else { value };
        writeln!(header, "#define {} {}", key, value).unwrap();
    }
    header
}

fn c_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(c_files(&path));
        } else if path.extension().is_some_and(|e| e == "c") {
            files.push(path);
        }
    }
    files
}

/// Drops Windows' `\\?\` extended-length prefix from a canonicalised path.
///
/// `Path::canonicalize` always returns the verbatim form on Windows, and it
/// poisons everything downstream: `cc` shortens long command lines by making
/// source paths relative, and relative-ising a `\\?\C:\...` path produced
/// bare names like `\\lv_group.c`, so MSVC could not open a single LVGL
/// source file. Every path here is far short of MAX_PATH, so the prefix buys
/// nothing. No-op on Unix.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        // UNC shares canonicalise to `\\?\UNC\server\share`; turning that
        // into `UNC\server\share` would be wrong, so leave it alone.
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest.to_string()),
        _ => path,
    }
}
