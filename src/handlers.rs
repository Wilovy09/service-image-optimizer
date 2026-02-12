use crate::config::{AppConfig, CorsConfig};
use crate::models::*;
use crate::services::ImageCompressionService;
use base64::{Engine as _, engine::general_purpose};
use bytes::Bytes;
use futures_util::stream;
use hyper::header::{CONTENT_TYPE, HeaderMap};
use hyper::{Body, Method, Request, Response, StatusCode};
use lambda_runtime::{Error, LambdaEvent};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::convert::Infallible;

pub struct ImageHandler {
    compression_service: ImageCompressionService,
    cors_config: CorsConfig,
    max_image_size: usize,
}

impl ImageHandler {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            compression_service: ImageCompressionService::new(),
            cors_config: config.cors.clone(),
            max_image_size: config.compression.max_image_size,
        }
    }

    pub async fn handle_http_request(
        &self,
        req: Request<Body>,
    ) -> Result<Response<Body>, Infallible> {
        let origin = req
            .headers()
            .get("origin")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if req.method() == Method::OPTIONS {
            return Ok(self.create_cors_response(StatusCode::OK, Body::empty(), origin.as_deref()));
        }

        if req.method() == Method::POST && req.uri().path() == "/optimize" {
            let content_type = self.get_content_type(req.headers());
            if self.is_multipart_content_type(content_type.as_deref()) {
                match self.process_multipart_optimize(req, &content_type).await {
                    Ok(result) => {
                        let resp_content_type = self
                            .content_type_for_format(&result.output_format)
                            .to_string();
                        Ok(self.create_cors_binary_response(
                            StatusCode::OK,
                            result.optimized_bytes,
                            origin.as_deref(),
                            resp_content_type.as_str(),
                            result.original_size,
                            result.optimized_size,
                            &result.original_format,
                        ))
                    }
                    Err(error_body) => Ok(self.create_cors_response(
                        StatusCode::BAD_REQUEST,
                        Body::from(self.wrap_error_json(error_body)),
                        origin.as_deref(),
                    )),
                }
            } else {
                match self.process_http_request_body(req).await {
                    Ok(response_body) => Ok(self.create_cors_response(
                        StatusCode::OK,
                        Body::from(response_body),
                        origin.as_deref(),
                    )),
                    Err(error_body) => Ok(self.create_cors_response(
                        StatusCode::BAD_REQUEST,
                        Body::from(self.wrap_error_json(error_body)),
                        origin.as_deref(),
                    )),
                }
            }
        } else if req.method() == Method::POST && req.uri().path() == "/resize" {
            let content_type = self.get_content_type(req.headers());
            if self.is_multipart_content_type(content_type.as_deref()) {
                match self.process_multipart_resize(req, &content_type).await {
                    Ok(result) => {
                        let resp_content_type = self
                            .content_type_for_format(&result.output_format)
                            .to_string();
                        Ok(self.create_cors_binary_response(
                            StatusCode::OK,
                            result.optimized_bytes,
                            origin.as_deref(),
                            resp_content_type.as_str(),
                            result.original_size,
                            result.optimized_size,
                            &result.original_format,
                        ))
                    }
                    Err(error_body) => Ok(self.create_cors_response(
                        StatusCode::BAD_REQUEST,
                        Body::from(self.wrap_error_json(error_body)),
                        origin.as_deref(),
                    )),
                }
            } else {
                Ok(self.create_cors_response(
                    StatusCode::BAD_REQUEST,
                    Body::from(
                        self.wrap_error_json(
                            "Content-Type debe ser multipart/form-data".to_string(),
                        ),
                    ),
                    origin.as_deref(),
                ))
            }
        } else if req.method() == Method::POST && req.uri().path() == "/optimize-binary" {
            match self.process_binary_request(req).await {
                Ok(response_body) => Ok(self.create_cors_response(
                    StatusCode::OK,
                    Body::from(response_body),
                    origin.as_deref(),
                )),
                Err(error_body) => Ok(self.create_cors_response(
                    StatusCode::BAD_REQUEST,
                    Body::from(self.wrap_error_json(error_body)),
                    origin.as_deref(),
                )),
            }
        } else {
            Ok(Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap())
        }
    }

    pub async fn handle_lambda_event(&self, event: LambdaEvent<Value>) -> Result<Value, Error> {
        dotenv::dotenv().ok();

        let origin = event
            .payload
            .get("headers")
            .and_then(|h| h.get("origin").or_else(|| h.get("Origin")))
            .and_then(|v| v.as_str());

        let method = self.get_event_method(&event.payload);
        let path = self.get_event_path(&event.payload);

        if method.as_deref() == Some("OPTIONS") {
            return Ok(json!({
                "statusCode": 200,
                "headers": self.get_cors_headers(origin),
                "body": ""
            }));
        }

        let content_type = self.get_event_header(&event.payload, "content-type");
        let is_multipart = self.is_multipart_content_type(content_type.as_deref());
        let query_params = self.get_event_query_params(&event.payload);

        let body_bytes = self.get_event_body_bytes(&event.payload)?;

        match (method.as_deref(), path.as_deref()) {
            (Some("POST"), Some("/optimize")) => {
                if is_multipart {
                    match self
                        .process_multipart_optimize_bytes(
                            content_type.as_deref(),
                            body_bytes,
                            &query_params,
                        )
                        .await
                    {
                        Ok(result) => {
                            let content_type = self
                                .content_type_for_format(&result.output_format)
                                .to_string();
                            Ok(self.create_lambda_binary_response(origin, result, content_type))
                        }
                        Err(error_body) => {
                            Ok(self.create_lambda_error_response(origin, error_body))
                        }
                    }
                } else {
                    let body_str = String::from_utf8_lossy(&body_bytes);
                    match self.process_request_body(&body_str).await {
                        Ok(response_json) => Ok(json!({
                            "statusCode": 200,
                            "headers": self.get_cors_headers(origin),
                            "body": response_json
                        })),
                        Err(error_json) => {
                            Ok(self.create_lambda_error_response(origin, error_json))
                        }
                    }
                }
            }
            (Some("POST"), Some("/resize")) => {
                if is_multipart {
                    match self
                        .process_multipart_resize_bytes(
                            content_type.as_deref(),
                            body_bytes,
                            &query_params,
                        )
                        .await
                    {
                        Ok(result) => {
                            let content_type = self
                                .content_type_for_format(&result.output_format)
                                .to_string();
                            Ok(self.create_lambda_binary_response(origin, result, content_type))
                        }
                        Err(error_body) => {
                            Ok(self.create_lambda_error_response(origin, error_body))
                        }
                    }
                } else {
                    Ok(self.create_lambda_error_response(
                        origin,
                        "Content-Type debe ser multipart/form-data".to_string(),
                    ))
                }
            }
            _ => Ok(json!({
                "statusCode": 404,
                "headers": self.get_cors_headers(origin),
                "body": "Not Found"
            })),
        }
    }

    async fn process_http_request_body(&self, req: Request<Body>) -> Result<String, String> {
        let body_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .map_err(|_| "Error reading request body".to_string())?;

        let body_str = String::from_utf8_lossy(&body_bytes);
        self.process_request_body(&body_str).await
    }

    async fn process_binary_request(&self, req: Request<Body>) -> Result<String, String> {
        let uri = req.uri();
        let query = uri.query().unwrap_or("");
        let params: std::collections::HashMap<String, String> =
            url::form_urlencoded::parse(query.as_bytes())
                .into_owned()
                .collect();

        let quality = params
            .get("quality")
            .and_then(|q| q.parse().ok())
            .unwrap_or(85u8);

        let format = params
            .get("format")
            .cloned()
            .unwrap_or_else(|| "auto".to_string());

        let progressive = params
            .get("progressive")
            .and_then(|p| p.parse().ok())
            .unwrap_or(false);

        let aggressive = params
            .get("aggressive")
            .and_then(|a| a.parse().ok())
            .unwrap_or(false);

        // Leer body como bytes
        let image_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .map_err(|_| "Error reading image data".to_string())?;

        if image_bytes.is_empty() {
            return Err("No image data provided".to_string());
        }

        let image_base64 = general_purpose::STANDARD.encode(&image_bytes);

        let request = OptimizeRequest {
            image_data: image_base64,
            quality,
            format,
            progressive,
            aggressive,
        };

        let result = self
            .compression_service
            .optimize_image_with_limit(request, self.max_image_size)
            .await?;
        let response = self.compression_service.create_response(result);

        serde_json::to_string(&response).map_err(|_| "Error serializing response".to_string())
    }

    async fn process_request_body(&self, body_str: &str) -> Result<String, String> {
        let request = if let Ok(req) = serde_json::from_str::<OptimizeRequest>(body_str) {
            req
        } else {
            OptimizeRequest {
                image_data: body_str.to_string(),
                quality: 60,
                format: "auto".to_string(),
                progressive: true,
                aggressive: true,
            }
        };

        let result = self
            .compression_service
            .optimize_image_with_limit(request, self.max_image_size)
            .await?;
        let response = self.compression_service.create_response(result);

        serde_json::to_string(&response).map_err(|_| "Error serializing response".to_string())
    }

    async fn process_multipart_optimize(
        &self,
        req: Request<Body>,
        content_type: &Option<String>,
    ) -> Result<BinaryCompressionResult, String> {
        let query_params = self.parse_query_params(req.uri().query().unwrap_or(""));
        let options = self.build_transform_options(&query_params, None)?;

        let content_type = content_type
            .as_deref()
            .ok_or_else(|| "Content-Type requerido".to_string())?;

        let file_bytes = self
            .extract_multipart_file_from_body(content_type, req.into_body())
            .await?;

        self.validate_image_size(&file_bytes)?;

        self.compression_service
            .optimize_image_bytes(&file_bytes, &options)
            .await
    }

    async fn process_multipart_resize(
        &self,
        req: Request<Body>,
        content_type: &Option<String>,
    ) -> Result<BinaryCompressionResult, String> {
        let query_params = self.parse_query_params(req.uri().query().unwrap_or(""));
        let resize = self.parse_resize_options(&query_params)?;
        let options = self.build_transform_options(&query_params, Some(resize))?;

        let content_type = content_type
            .as_deref()
            .ok_or_else(|| "Content-Type requerido".to_string())?;

        let file_bytes = self
            .extract_multipart_file_from_body(content_type, req.into_body())
            .await?;

        self.validate_image_size(&file_bytes)?;

        self.compression_service
            .optimize_image_bytes(&file_bytes, &options)
            .await
    }

    async fn process_multipart_optimize_bytes(
        &self,
        content_type: Option<&str>,
        body_bytes: Vec<u8>,
        query_params: &HashMap<String, String>,
    ) -> Result<BinaryCompressionResult, String> {
        let content_type = content_type.ok_or_else(|| "Content-Type requerido".to_string())?;
        let options = self.build_transform_options(query_params, None)?;

        let file_bytes = self
            .extract_multipart_file_from_bytes(content_type, body_bytes)
            .await?;

        self.validate_image_size(&file_bytes)?;

        self.compression_service
            .optimize_image_bytes(&file_bytes, &options)
            .await
    }

    async fn process_multipart_resize_bytes(
        &self,
        content_type: Option<&str>,
        body_bytes: Vec<u8>,
        query_params: &HashMap<String, String>,
    ) -> Result<BinaryCompressionResult, String> {
        let content_type = content_type.ok_or_else(|| "Content-Type requerido".to_string())?;
        let resize = self.parse_resize_options(query_params)?;
        let options = self.build_transform_options(query_params, Some(resize))?;

        let file_bytes = self
            .extract_multipart_file_from_bytes(content_type, body_bytes)
            .await?;

        self.validate_image_size(&file_bytes)?;

        self.compression_service
            .optimize_image_bytes(&file_bytes, &options)
            .await
    }

    fn create_cors_response(
        &self,
        status: StatusCode,
        body: Body,
        origin: Option<&str>,
    ) -> Response<Body> {
        let allowed_origin = self.get_allowed_origin(origin);

        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", allowed_origin)
            .header("Access-Control-Allow-Methods", "POST, OPTIONS")
            .header(
                "Access-Control-Allow-Headers",
                "Content-Type, Authorization",
            )
            .body(body)
            .unwrap()
    }

    fn create_cors_binary_response(
        &self,
        status: StatusCode,
        body_bytes: Vec<u8>,
        origin: Option<&str>,
        content_type: &str,
        original_size: usize,
        optimized_size: usize,
        original_format: &str,
    ) -> Response<Body> {
        let allowed_origin = self.get_allowed_origin(origin);

        Response::builder()
            .status(status)
            .header("Content-Type", content_type)
            .header("Access-Control-Allow-Origin", allowed_origin)
            .header("Access-Control-Allow-Methods", "POST, OPTIONS")
            .header(
                "Access-Control-Allow-Headers",
                "Content-Type, Authorization",
            )
            .header(
                "Access-Control-Expose-Headers",
                "X-Original-Size, X-Optimized-Size, X-Original-Format",
            )
            .header("X-Original-Size", original_size.to_string())
            .header("X-Optimized-Size", optimized_size.to_string())
            .header("X-Original-Format", original_format)
            .body(Body::from(body_bytes))
            .unwrap()
    }

    fn get_cors_headers(&self, origin: Option<&str>) -> serde_json::Map<String, Value> {
        let mut headers = serde_json::Map::new();
        let allowed_origin = self.get_allowed_origin(origin);

        headers.insert("Content-Type".to_string(), json!("application/json"));
        headers.insert(
            "Access-Control-Allow-Origin".to_string(),
            json!(allowed_origin),
        );
        headers.insert(
            "Access-Control-Allow-Methods".to_string(),
            json!("POST, OPTIONS"),
        );
        headers.insert(
            "Access-Control-Allow-Headers".to_string(),
            json!("Content-Type, Authorization"),
        );
        headers
    }

    fn get_cors_headers_with_content_type(
        &self,
        origin: Option<&str>,
        content_type: &str,
    ) -> serde_json::Map<String, Value> {
        let mut headers = serde_json::Map::new();
        let allowed_origin = self.get_allowed_origin(origin);

        headers.insert("Content-Type".to_string(), json!(content_type));
        headers.insert(
            "Access-Control-Allow-Origin".to_string(),
            json!(allowed_origin),
        );
        headers.insert(
            "Access-Control-Allow-Methods".to_string(),
            json!("POST, OPTIONS"),
        );
        headers.insert(
            "Access-Control-Allow-Headers".to_string(),
            json!("Content-Type, Authorization"),
        );
        headers.insert(
            "Access-Control-Expose-Headers".to_string(),
            json!("X-Original-Size, X-Optimized-Size, X-Original-Format"),
        );
        headers
    }

    fn get_allowed_origin(&self, origin: Option<&str>) -> String {
        match origin {
            Some(origin_value) => {
                if self.cors_config.allowed_origins.contains(&"*".to_string()) {
                    "*".to_string()
                } else if self
                    .cors_config
                    .allowed_origins
                    .contains(&origin_value.to_string())
                {
                    origin_value.to_string()
                } else {
                    "null".to_string()
                }
            }
            None => {
                if self.cors_config.allowed_origins.contains(&"*".to_string()) {
                    "*".to_string()
                } else {
                    "null".to_string()
                }
            }
        }
    }

    fn get_content_type(&self, headers: &HeaderMap) -> Option<String> {
        headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string())
    }

    fn is_multipart_content_type(&self, content_type: Option<&str>) -> bool {
        content_type
            .map(|value| value.to_lowercase().starts_with("multipart/form-data"))
            .unwrap_or(false)
    }

    async fn extract_multipart_file_from_body(
        &self,
        content_type: &str,
        body: Body,
    ) -> Result<Vec<u8>, String> {
        let boundary = multer::parse_boundary(content_type)
            .map_err(|_| "Boundary invalido en Content-Type".to_string())?;
        let mut multipart = multer::Multipart::new(body, boundary);

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|_| "Error leyendo multipart".to_string())?
        {
            if field.name() == Some("file") {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| "Error leyendo archivo".to_string())?;
                return Ok(bytes.to_vec());
            }
        }

        Err("No se encontro archivo en multipart".to_string())
    }

    async fn extract_multipart_file_from_bytes(
        &self,
        content_type: &str,
        body_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        let boundary = multer::parse_boundary(content_type)
            .map_err(|_| "Boundary invalido en Content-Type".to_string())?;
        let stream = stream::once(async move { Ok::<Bytes, Infallible>(Bytes::from(body_bytes)) });
        let mut multipart = multer::Multipart::new(stream, boundary);

        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|_| "Error leyendo multipart".to_string())?
        {
            if field.name() == Some("file") {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|_| "Error leyendo archivo".to_string())?;
                return Ok(bytes.to_vec());
            }
        }

        Err("No se encontro archivo en multipart".to_string())
    }

    fn validate_image_size(&self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > self.max_image_size {
            return Err("Payload demasiado grande".to_string());
        }
        Ok(())
    }

    fn parse_query_params(&self, query: &str) -> HashMap<String, String> {
        url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect()
    }

    fn build_transform_options(
        &self,
        params: &HashMap<String, String>,
        resize: Option<ResizeOptions>,
    ) -> Result<TransformOptions, String> {
        let quality = self.parse_quality(params.get("q"))?;
        let black_and_white = self.parse_bool(params.get("bw"))?;
        let border_radius = self.parse_optional_u32(params.get("br"))?.unwrap_or(0);
        let output_format = self.parse_output_format(params.get("f"))?;

        Ok(TransformOptions {
            quality,
            black_and_white,
            border_radius,
            resize,
            output_format,
        })
    }

    fn parse_resize_options(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<ResizeOptions, String> {
        let width = self.parse_optional_u32(params.get("w"))?;
        let height = self.parse_optional_u32(params.get("h"))?;

        if width.is_none() && height.is_none() {
            return Err("Debes enviar w o h".to_string());
        }

        let mode = match params.get("t").map(|v| v.as_str()).unwrap_or("fit") {
            "fit" => ResizeMode::Fit,
            "fill" => ResizeMode::Fill,
            "force" => ResizeMode::Force,
            _ => return Err("Parametro t invalido".to_string()),
        };

        Ok(ResizeOptions {
            width,
            height,
            mode,
        })
    }

    fn parse_output_format(&self, value: Option<&String>) -> Result<Option<String>, String> {
        match value {
            None => Ok(None),
            Some(raw) => match raw.to_lowercase().as_str() {
                "jpeg" | "jpg" => Ok(Some("jpeg".to_string())),
                "png" => Ok(Some("png".to_string())),
                "webp" => Ok(Some("webp".to_string())),
                _ => Err("Parametro f invalido (jpeg, png, webp)".to_string()),
            },
        }
    }

    fn parse_quality(&self, value: Option<&String>) -> Result<u8, String> {
        let quality = value.and_then(|v| v.parse::<u8>().ok()).unwrap_or(85);

        if !(1..=100).contains(&quality) {
            return Err("Parametro q invalido".to_string());
        }

        Ok(quality)
    }

    fn parse_bool(&self, value: Option<&String>) -> Result<bool, String> {
        match value {
            None => Ok(false),
            Some(raw) => match raw.to_lowercase().as_str() {
                "true" | "1" | "yes" => Ok(true),
                "false" | "0" | "no" => Ok(false),
                _ => Err("Parametro bw invalido".to_string()),
            },
        }
    }

    fn parse_optional_u32(&self, value: Option<&String>) -> Result<Option<u32>, String> {
        match value {
            None => Ok(None),
            Some(raw) => raw
                .parse::<u32>()
                .map(Some)
                .map_err(|_| "Parametro numerico invalido".to_string()),
        }
    }

    fn content_type_for_format(&self, format: &str) -> &str {
        match format {
            "jpeg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "tiff" => "image/tiff",
            _ => "application/octet-stream",
        }
    }

    fn get_event_method(&self, payload: &Value) -> Option<String> {
        payload
            .get("requestContext")
            .and_then(|ctx| ctx.get("http"))
            .and_then(|http| http.get("method"))
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .or_else(|| {
                payload
                    .get("httpMethod")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
            })
    }

    fn get_event_path(&self, payload: &Value) -> Option<String> {
        payload
            .get("rawPath")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .or_else(|| {
                payload
                    .get("path")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
            })
    }

    fn get_event_header(&self, payload: &Value, name: &str) -> Option<String> {
        let headers = payload.get("headers")?;
        let name_lower = name.to_lowercase();
        headers.as_object().and_then(|map| {
            map.iter().find_map(|(key, value)| {
                if key.to_lowercase() == name_lower {
                    value.as_str().map(|v| v.to_string())
                } else {
                    None
                }
            })
        })
    }

    fn get_event_query_params(&self, payload: &Value) -> HashMap<String, String> {
        let mut params = HashMap::new();

        if let Some(raw) = payload.get("rawQueryString").and_then(|v| v.as_str()) {
            if !raw.is_empty() {
                params.extend(self.parse_query_params(raw));
                return params;
            }
        }

        if let Some(map) = payload
            .get("queryStringParameters")
            .and_then(|v| v.as_object())
        {
            for (key, value) in map {
                if let Some(value_str) = value.as_str() {
                    params.insert(key.clone(), value_str.to_string());
                }
            }
        }

        params
    }

    fn get_event_body_bytes(&self, payload: &Value) -> Result<Vec<u8>, String> {
        let body = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");

        let is_base64 = payload
            .get("isBase64Encoded")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if is_base64 {
            general_purpose::STANDARD
                .decode(body)
                .map_err(|_| "Body base64 invalido".to_string())
        } else {
            Ok(body.as_bytes().to_vec())
        }
    }

    fn create_lambda_binary_response(
        &self,
        origin: Option<&str>,
        result: BinaryCompressionResult,
        content_type: String,
    ) -> Value {
        let mut headers = self.get_cors_headers_with_content_type(origin, &content_type);
        headers.insert(
            "X-Original-Size".to_string(),
            json!(result.original_size.to_string()),
        );
        headers.insert(
            "X-Optimized-Size".to_string(),
            json!(result.optimized_size.to_string()),
        );
        headers.insert(
            "X-Original-Format".to_string(),
            json!(result.original_format),
        );

        let body = general_purpose::STANDARD.encode(&result.optimized_bytes);

        json!({
            "statusCode": 200,
            "headers": headers,
            "body": body,
            "isBase64Encoded": true
        })
    }

    fn create_lambda_error_response(&self, origin: Option<&str>, error_body: String) -> Value {
        json!({
            "statusCode": 400,
            "headers": self.get_cors_headers(origin),
            "body": self.wrap_error_json(error_body)
        })
    }

    fn wrap_error_json(&self, message: String) -> String {
        json!({ "error": message }).to_string()
    }
}
