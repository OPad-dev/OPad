use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    // desktop/crates/opad-protocol -> repo root; protocol/osupad.proto is the
    // same schema nanopb generates the firmware's osupad_* types from
    let proto_dir = manifest_dir
        .ancestors()
        .nth(3)
        .ok_or("opad-protocol is expected at desktop/crates/opad-protocol")?
        .join("protocol");
    let proto_file = proto_dir.join("osupad.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    // protox parses the schema in-process, so building needs no protoc install
    let descriptors = protox::compile([&proto_file], [&proto_dir])?;
    prost_build::Config::new().compile_fds(descriptors)?;

    Ok(())
}
