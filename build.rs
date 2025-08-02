fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .out_dir("src") // 生成到 src/api.rs
        .compile_protos(&["src/proto/api.proto"], &["src/proto"])?;
    Ok(())
}