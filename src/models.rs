use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone)]
pub struct OptimizeRequest {
    pub image_data: String, // base64 encoded image
    #[serde(default = "default_quality")]
    pub quality: u8, // 1-100, default 75
    #[serde(default = "default_format")]
    pub format: String, // "jpeg", "png", "webp", "auto"
    #[serde(default)]
    pub progressive: bool, // Progressive JPEG
    #[serde(default)]
    pub aggressive: bool, // Aggressive compression
}

#[derive(Serialize, Debug, Clone)]
pub struct OptimizeResponse {
    pub optimized_image: String, // base64 encoded optimized image
    pub original_size: usize,
    pub optimized_size: usize,
    pub compression_ratio: f64,
    pub original_format: String,
    pub output_format: String,
    pub quality_used: u8,
}

#[derive(Serialize, Debug)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Clone)]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub format: String,
}

#[derive(Debug, Clone)]
pub struct CompressionResult {
    pub optimized_bytes: Vec<u8>,
    pub original_size: usize,
    pub optimized_size: usize,
    pub compression_ratio: f64,
    pub original_format: String,
    pub output_format: String,
    pub quality_used: u8,
}

fn default_quality() -> u8 {
    75
}

fn default_format() -> String {
    "auto".to_string()
}
