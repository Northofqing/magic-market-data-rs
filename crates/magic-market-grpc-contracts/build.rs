#![forbid(unsafe_code)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc = protoc_bin_vendored::protoc_bin_path()?;
    std::env::set_var("PROTOC", protoc);
    tonic_prost_build::configure()
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR")?).join("magic.market.v1.bin"),
        )
        .compile_protos(&["proto/magic/market/v1/market.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/magic/market/v1/market.proto");
    Ok(())
}
