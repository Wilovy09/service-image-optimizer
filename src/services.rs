use crate::models::*;
use crate::utils::decode_base64;
use base64::{Engine as _, engine::general_purpose};
use image::{
    DynamicImage, GenericImageView, ImageEncoder, ImageFormat, Rgba, RgbaImage,
    codecs::jpeg::JpegEncoder, codecs::png::PngEncoder,
};
use std::io::Cursor;

pub struct ImageCompressionService;

impl ImageCompressionService {
    pub fn new() -> Self {
        Self
    }

    pub async fn optimize_image_with_limit(
        &self,
        request: OptimizeRequest,
        max_image_size: usize,
    ) -> Result<CompressionResult, String> {
        let image_data = decode_base64(&request.image_data)
            .map_err(|_| "Datos de imagen base64 inválidos".to_string())?;

        if image_data.bytes.len() > max_image_size {
            return Err("Payload demasiado grande".to_string());
        }

        let original_format = self.detect_image_format(&image_data.bytes)?;
        let output_format = self.determine_output_format(&request.format, &original_format);

        let img = image::load_from_memory(&image_data.bytes)
            .map_err(|_| "Formato de imagen no soportado".to_string())?;

        let effective_quality = if request.aggressive {
            request.quality.min(60)
        } else {
            request.quality
        };

        let result_bytes = match output_format.as_str() {
            "jpeg" => self.compress_jpeg_with_quality(&img, effective_quality)?,
            "png" => self.compress_png(&image_data.bytes)?,
            "webp" => self.compress_webp_with_quality(&img, effective_quality)?,
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
            quality_used: effective_quality,
        })
    }

    pub async fn optimize_image_bytes(
        &self,
        original_bytes: &[u8],
        options: &TransformOptions,
    ) -> Result<BinaryCompressionResult, String> {
        self.process_image_bytes(original_bytes, options)
    }

    fn detect_image_format(&self, bytes: &[u8]) -> Result<String, String> {
        match image::guess_format(bytes) {
            Ok(ImageFormat::Jpeg) => Ok("jpeg".to_string()),
            Ok(ImageFormat::Png) => Ok("png".to_string()),
            Ok(ImageFormat::WebP) => Ok("webp".to_string()),
            Ok(ImageFormat::Gif) => Ok("gif".to_string()),
            Ok(ImageFormat::Bmp) => Ok("bmp".to_string()),
            Ok(ImageFormat::Tiff) => Ok("tiff".to_string()),
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

    fn compress_jpeg_with_quality(
        &self,
        img: &DynamicImage,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        let rgb_img = img.to_rgb8();

        let encoder = JpegEncoder::new_with_quality(&mut cursor, quality);

        encoder
            .write_image(
                rgb_img.as_raw(),
                rgb_img.width(),
                rgb_img.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|_| "Error comprimiendo JPEG".to_string())?;

        Ok(buffer)
    }

    fn compress_png(&self, original_bytes: &[u8]) -> Result<Vec<u8>, String> {
        let img = image::load_from_memory(original_bytes)
            .map_err(|_| "Error cargando PNG".to_string())?;

        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        let encoder = PngEncoder::new(&mut cursor);

        encoder
            .write_image(
                img.as_bytes(),
                img.width(),
                img.height(),
                img.color().into(),
            )
            .map_err(|_| "Error comprimiendo PNG".to_string())?;

        Ok(buffer)
    }

    fn compress_png_from_image(&self, img: &DynamicImage) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);

        let encoder = PngEncoder::new(&mut cursor);

        encoder
            .write_image(
                img.as_bytes(),
                img.width(),
                img.height(),
                img.color().into(),
            )
            .map_err(|_| "Error comprimiendo PNG".to_string())?;

        Ok(buffer)
    }

    fn compress_webp_with_quality(
        &self,
        img: &DynamicImage,
        quality: u8,
    ) -> Result<Vec<u8>, String> {
        let rgb_img = img.to_rgb8();

        let encoder = webp::Encoder::from_rgb(&rgb_img, rgb_img.width(), rgb_img.height());
        let encoded = encoder.encode(quality as f32);

        Ok(encoded.to_vec())
    }

    fn process_image_bytes(
        &self,
        original_bytes: &[u8],
        options: &TransformOptions,
    ) -> Result<BinaryCompressionResult, String> {
        let original_format = self.detect_image_format(original_bytes)?;
        let mut output_format = options
            .output_format
            .clone()
            .unwrap_or_else(|| original_format.clone());

        let mut img = image::load_from_memory(original_bytes)
            .map_err(|_| "Formato de imagen no soportado".to_string())?;

        if let Some(resize) = &options.resize {
            img = self.resize_image(&img, resize)?;
        }

        if options.black_and_white {
            img = self.apply_black_and_white(&img);
        }

        if options.border_radius > 0 {
            img = self.apply_border_radius(&img, options.border_radius);
            // Transparency requires a format that supports alpha
            if output_format == "jpeg" {
                output_format = "png".to_string();
            }
        }

        let result_bytes = match output_format.as_str() {
            "jpeg" => self.compress_jpeg_with_quality(&img, options.quality)?,
            "png" => self.compress_png_from_image(&img)?,
            "webp" => self.compress_webp_with_quality(&img, options.quality)?,
            "gif" => self.encode_with_format(&img, ImageFormat::Gif)?,
            "bmp" => self.encode_with_format(&img, ImageFormat::Bmp)?,
            "tiff" => self.encode_with_format(&img, ImageFormat::Tiff)?,
            _ => return Err("Formato de salida no soportado".to_string()),
        };

        let original_size = original_bytes.len();
        let optimized_size = result_bytes.len();

        Ok(BinaryCompressionResult {
            optimized_bytes: result_bytes,
            original_size,
            optimized_size,
            original_format,
            output_format,
        })
    }

    fn resize_image(
        &self,
        img: &DynamicImage,
        options: &ResizeOptions,
    ) -> Result<DynamicImage, String> {
        let (orig_w, orig_h) = img.dimensions();

        if options.width.is_none() && options.height.is_none() {
            return Err("Debes enviar w o h".to_string());
        }

        let (target_w, target_h) = match (options.width, options.height) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => {
                let h = ((w as f32 / orig_w as f32) * orig_h as f32).round() as u32;
                (w, h.max(1))
            }
            (None, Some(h)) => {
                let w = ((h as f32 / orig_h as f32) * orig_w as f32).round() as u32;
                (w.max(1), h)
            }
            (None, None) => (orig_w, orig_h),
        };

        if target_w == 0 || target_h == 0 {
            return Err("Dimensiones invalidas".to_string());
        }

        match options.mode {
            ResizeMode::Force => {
                Ok(img.resize_exact(target_w, target_h, image::imageops::FilterType::Lanczos3))
            }
            ResizeMode::Fit => {
                let resized = img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3);
                Ok(resized)
            }
            ResizeMode::Fill => {
                let scale = f32::max(
                    target_w as f32 / orig_w as f32,
                    target_h as f32 / orig_h as f32,
                );
                let scaled_w = (orig_w as f32 * scale).round() as u32;
                let scaled_h = (orig_h as f32 * scale).round() as u32;
                let resized = img.resize_exact(
                    scaled_w.max(1),
                    scaled_h.max(1),
                    image::imageops::FilterType::Lanczos3,
                );

                let x = (scaled_w.saturating_sub(target_w)) / 2;
                let y = (scaled_h.saturating_sub(target_h)) / 2;
                let cropped =
                    image::imageops::crop_imm(&resized, x, y, target_w, target_h).to_image();
                Ok(DynamicImage::ImageRgba8(cropped))
            }
        }
    }

    fn apply_black_and_white(&self, img: &DynamicImage) -> DynamicImage {
        let mut rgba = img.to_rgba8();
        for pixel in rgba.pixels_mut() {
            let [r, g, b, a] = pixel.0;
            let l = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as u8;
            *pixel = Rgba([l, l, l, a]);
        }
        DynamicImage::ImageRgba8(rgba)
    }

    fn apply_border_radius(&self, img: &DynamicImage, radius: u32) -> DynamicImage {
        let mut rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let radius = radius.min(width / 2).min(height / 2) as i32;

        if radius <= 0 {
            return DynamicImage::ImageRgba8(rgba);
        }

        let r2 = radius * radius;
        let w = width as i32;
        let h = height as i32;

        for y in 0..radius {
            for x in 0..radius {
                if self.is_outside_circle(x, y, radius, r2) {
                    self.set_alpha(&mut rgba, x, y, 0);
                    self.set_alpha(&mut rgba, w - 1 - x, y, 0);
                    self.set_alpha(&mut rgba, x, h - 1 - y, 0);
                    self.set_alpha(&mut rgba, w - 1 - x, h - 1 - y, 0);
                }
            }
        }

        DynamicImage::ImageRgba8(rgba)
    }

    fn is_outside_circle(&self, x: i32, y: i32, radius: i32, r2: i32) -> bool {
        let dx = x - radius + 1;
        let dy = y - radius + 1;
        dx * dx + dy * dy > r2
    }

    fn set_alpha(&self, img: &mut RgbaImage, x: i32, y: i32, alpha: u8) {
        if x < 0 || y < 0 {
            return;
        }

        let (width, height) = img.dimensions();
        let x = x as u32;
        let y = y as u32;
        if x >= width || y >= height {
            return;
        }

        let mut pixel = *img.get_pixel(x, y);
        pixel.0[3] = alpha;
        img.put_pixel(x, y, pixel);
    }

    fn encode_with_format(
        &self,
        img: &DynamicImage,
        format: ImageFormat,
    ) -> Result<Vec<u8>, String> {
        let mut buffer = Vec::new();
        let mut cursor = Cursor::new(&mut buffer);
        img.write_to(&mut cursor, format)
            .map_err(|_| "Error codificando imagen".to_string())?;
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
