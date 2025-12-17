use crate::models::*;
use crate::utils::decode_base64;
use base64::{Engine as _, engine::general_purpose};
use image::{DynamicImage, ImageEncoder, ImageFormat, codecs::jpeg::JpegEncoder};
use oxipng::{Options, StripChunks, optimize_from_memory};
use std::io::Cursor;
use std::time::Duration;

pub struct ImageCompressionService;

impl ImageCompressionService {
    pub fn new() -> Self {
        Self
    }

    pub async fn optimize_image(
        &self,
        request: OptimizeRequest,
    ) -> Result<CompressionResult, String> {
        let image_data = decode_base64(&request.image_data)
            .map_err(|_| "Datos de imagen base64 inválidos".to_string())?;

        let original_format = self.detect_image_format(&image_data.bytes)?;
        let output_format = self.determine_output_format(&request.format, &original_format);

        let img = image::load_from_memory(&image_data.bytes)
            .map_err(|_| "Formato de imagen no soportado".to_string())?;

        let result_bytes = match output_format.as_str() {
            "jpeg" => self.compress_jpeg(&img, &request)?,
            "png" => self.compress_png(&image_data.bytes, &request)?,
            "webp" => self.compress_webp(&img, &request)?,
            _ => return Err("Formato de salida no soportado".to_string()),
        };

        let original_size = image_data.bytes.len();
        let optimized_size = result_bytes.len();
        let compression_ratio =
            ((original_size as f64 - optimized_size as f64) / original_size as f64) * 100.0;

        Ok(CompressionResult {
            optimized_bytes: result_bytes,
            original_size,
            optimized_size,
            compression_ratio,
            original_format,
            output_format,
            quality_used: request.quality,
        })
    }

    fn detect_image_format(&self, bytes: &[u8]) -> Result<String, String> {
        match image::guess_format(bytes) {
            Ok(ImageFormat::Jpeg) => Ok("jpeg".to_string()),
            Ok(ImageFormat::Png) => Ok("png".to_string()),
            Ok(ImageFormat::WebP) => Ok("webp".to_string()),
            Ok(format) => Ok(format!("{:?}", format).to_lowercase()),
            Err(_) => Err("No se pudo detectar el formato de imagen".to_string()),
        }
    }

    fn determine_output_format(&self, requested: &str, original: &str) -> String {
        match requested {
            "auto" => match original {
                "png" => "jpeg".to_string(),
                "jpeg" => "jpeg".to_string(),
                _ => "jpeg".to_string(),
            },
            format => format.to_string(),
        }
    }

    fn compress_jpeg(
        &self,
        img: &DynamicImage,
        request: &OptimizeRequest,
    ) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        let rgb_img = img.to_rgb8();

        let encoder = JpegEncoder::new_with_quality(&mut cursor, request.quality);

        encoder
            .write_image(
                rgb_img.as_raw(),
                rgb_img.width(),
                rgb_img.height(),
                image::ColorType::Rgb8,
            )
            .map_err(|_| "Error comprimiendo JPEG".to_string())?;

        Ok(buffer)
    }

    fn compress_png(
        &self,
        original_bytes: &[u8],
        request: &OptimizeRequest,
    ) -> Result<Vec<u8>, String> {
        let mut options = Options::default();
        options.optimize_alpha = true;
        options.strip = StripChunks::Safe;
        options.interlace = Some(false);
        options.timeout = Some(Duration::from_secs(10));
        options.max_decompressed_size = Some(50 * 1024 * 1024);

        if request.aggressive {
            options.strip = StripChunks::All;
        }

        optimize_from_memory(original_bytes, &options)
            .map_err(|_| "Error optimizando PNG".to_string())
    }

    fn compress_webp(
        &self,
        img: &DynamicImage,
        _request: &OptimizeRequest,
    ) -> Result<Vec<u8>, String> {
        let rgb_img = img.to_rgb8();
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        image::write_buffer_with_format(
            &mut cursor,
            &rgb_img,
            rgb_img.width(),
            rgb_img.height(),
            image::ColorType::Rgb8,
            ImageFormat::WebP,
        )
        .map_err(|_| "Error comprimiendo WebP".to_string())?;

        Ok(buffer)
    }

    pub fn create_response(&self, result: CompressionResult) -> OptimizeResponse {
        let optimized_base64 = general_purpose::STANDARD.encode(&result.optimized_bytes);

        OptimizeResponse {
            optimized_image: optimized_base64,
            original_size: result.original_size,
            optimized_size: result.optimized_size,
            compression_ratio: result.compression_ratio,
            original_format: result.original_format,
            output_format: result.output_format,
            quality_used: result.quality_used,
        }
    }
}
