fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 시스템 protoc 설치 없이 빌드되도록 vendored 바이너리를 사용한다.
    let protoc = protoc_bin_vendored::protoc_bin_path()?;

    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);
    config.compile_protos(&["../proto/chat/v1/chat.proto"], &["../proto"])?;

    Ok(())
}
