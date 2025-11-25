fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .build_server(true)
        .out_dir("src/pb")
        .compile_protos(&["proto/ankr.proto"],&["proto/"],)?;
    Ok(())
}
    
    