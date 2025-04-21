use tokio::process::Command;

#[derive(Copy, Clone)]
pub enum CompressionMethod {
    FASTEST = 0,
    FASTER = 1,
    FAST = 2,
    NORMAL = 3,
    SLOW = 4,
    SLOWER = 5,
    SLOWEST = 6,
}

pub async fn webp_install_check() -> bool {
    Command::new("cwebp")
        .args(["-version"])
        .output()
        .await
        .is_ok()
}

pub async fn webp_convert(compression_method: CompressionMethod, quality: usize, input_file_path: &str, output_file_path: &str) -> Result<(), &'static str> {
    let compression_method_str = (compression_method as usize).to_string();
    if quality > 100 { return Err("Quality must be between 0 and 100") }
    let quality_str = quality.to_string();
    let _ = Command::new("cwebp")
        .args(["-m", &compression_method_str, "-q", &quality_str, "-mt", "-af", "-progress", &input_file_path, "-o", &output_file_path])
        .output()
        .await
        .map_err(|_| "Failed to convert to WEBP")?;
    
    return Ok(());
}