use std::fmt;
use tokio::process::Command;

#[derive(clap::ValueEnum, Copy, Clone, Debug)]
pub enum ConversionMethod {
    Fastest = 0,
    Faster = 1,
    Fast = 2,
    Normal = 3,
    Slow = 4,
    Slower = 5,
    Slowest = 6,
}

impl fmt::Display for ConversionMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let str = format!("{:?}", self).to_lowercase();
        write!(f, "{str}")
    }
}

pub async fn webp_install_check() -> bool {
    Command::new("cwebp")
        .args(["-version"])
        .output()
        .await
        .is_ok()
}

pub async fn webp_convert(conversion_method: ConversionMethod, quality: usize, input_file_path: &str, output_file_path: &str) -> Result<(), &'static str> {
    let compression_method_str = (conversion_method as usize).to_string();
    if quality > 100 { return Err("Quality must be between 0 and 100") }
    let quality_str = quality.to_string();
    let _ = Command::new("cwebp")
        .args(["-m", &compression_method_str, "-q", &quality_str, "-mt", "-af", "-progress", input_file_path, "-o", output_file_path])
        .output()
        .await
        .map_err(|_| "Failed to convert to WEBP")?;
    
    Ok(())
}