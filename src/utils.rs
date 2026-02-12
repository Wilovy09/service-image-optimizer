use crate::models::ImageData;
use base64::{Engine as _, engine::general_purpose};

pub fn decode_base64(base64_data: &str) -> Result<ImageData, Box<dyn std::error::Error>> {
    let bytes = general_purpose::STANDARD.decode(base64_data)?;
    Ok(ImageData { bytes })
}
