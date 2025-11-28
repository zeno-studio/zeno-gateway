use std::io::Result;

fn main() -> Result<()> {
    tonic_prost_build::configure()
        .build_server(true)
        .out_dir("src/pb")
        // 可以添加更多的配置选项来控制生成的代码
        .compile_protos(&["proto/ankr.proto"], &["proto/"])?;
    Ok(())
}