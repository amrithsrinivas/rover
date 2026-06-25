fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("proto");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(
            std::path::Path::new(&std::env::var("OUT_DIR").unwrap())
                .join("file_descriptor_set.bin"),
        )
        .compile_protos(
            &[
                proto_dir.join("rover/v1/common.proto"),
                proto_dir.join("rover/v1/auth.proto"),
                proto_dir.join("rover/v1/server.proto"),
                proto_dir.join("rover/v1/app.proto"),
            ],
            &[proto_dir],
        )?;

    Ok(())
}
