use crate::models::ImageData;
use base64::{Engine as _, engine::general_purpose};

pub fn decode_base64(base64_data: &str) -> Result<ImageData, Box<dyn std::error::Error>> {
    let bytes = general_purpose::STANDARD.decode(base64_data)?;

    let format = detect_format_from_bytes(&bytes);

    Ok(ImageData { bytes, format })
}

fn detect_format_from_bytes(bytes: &[u8]) -> String {
    if bytes.len() < 8 {
        return "unknown".to_string();
    }

    if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return "png".to_string();
    }

    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "jpeg".to_string();
    }

    if bytes.len() > 12
        && bytes[0..4] == [0x52, 0x49, 0x46, 0x46]
        && bytes[8..12] == [0x57, 0x45, 0x42, 0x50]
    {
        return "webp".to_string();
    }

    "unknown".to_string()
}

pub fn format_file_size(bytes: usize) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f = bytes as f64;
    let unit_index = (bytes_f.log(THRESHOLD) as usize).min(UNITS.len() - 1);
    let size = bytes_f / THRESHOLD.powi(unit_index as i32);

    format!("{:.2} {}", size, UNITS[unit_index])
}
