use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("PROTOC").is_err() {
        for candidate in [
            r"C:\msys64\mingw64\bin\protoc.exe",
            r"C:\Espressif\tools\python\v5.5.2\venv\Scripts\protoc.exe",
        ] {
            if std::path::Path::new(candidate).exists() {
                std::env::set_var("PROTOC", candidate);
                break;
            }
        }
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let proto_dir = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap()
        .join("protocol");
    let proto_file = proto_dir.join("osupad.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    prost_build::Config::new().compile_protos(&[proto_file], &[proto_dir])?;

    Ok(())
}
