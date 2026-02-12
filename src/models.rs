use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug, Clone)]
pub struct OptimizeRequest {
    pub image_data: String,
    #[serde(default = "default_quality")]
    pub quality: u8, // 1-100, default 75
    #[serde(default = "default_format")]
    pub format: String, // "jpeg", "png", "webp", "auto"
    #[allow(dead_code)] // Kept for API compatibility; image crate lacks progressive JPEG support
    #[serde(default)]
    pub progressive: bool,
    #[serde(default)]
    pub aggressive: bool, // Aggressive compression
}

#[derive(Serialize, Debug, Clone)]
pub struct OptimizeResponse {
    pub optimized_image: String,
    pub original_size: usize,
    pub optimized_size: usize,
    pub compression_ratio: f64,
    pub original_format: String,
    pub output_format: String,
    pub quality_used: u8,
}

#[derive(Debug, Clone)]
pub struct ImageData {
    pub bytes: Vec<u8>,
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

#[derive(Debug, Clone, Copy)]
pub enum ResizeMode {
    Fit,
    Fill,
    Force,
}

#[derive(Debug, Clone)]
pub struct ResizeOptions {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mode: ResizeMode,
}

#[derive(Debug, Clone)]
pub struct TransformOptions {
    pub quality: u8,
    pub black_and_white: bool,
    pub border_radius: u32,
    pub resize: Option<ResizeOptions>,
    pub output_format: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BinaryCompressionResult {
    pub optimized_bytes: Vec<u8>,
    pub original_size: usize,
    pub optimized_size: usize,
    pub original_format: String,
    pub output_format: String,
}

fn default_quality() -> u8 {
    75
}

fn default_format() -> String {
    "auto".to_string()
}
