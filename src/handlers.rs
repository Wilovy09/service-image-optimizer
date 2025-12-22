use crate::config::{AppConfig, CorsConfig};
use crate::models::*;
use crate::services::ImageCompressionService;
use base64::{Engine as _, engine::general_purpose};
use hyper::{Body, Method, Request, Response, StatusCode};
use lambda_runtime::{Error, LambdaEvent};
use serde_json::{Value, json};
use std::convert::Infallible;

pub struct ImageHandler {
    compression_service: ImageCompressionService,
    cors_config: CorsConfig,
}

impl ImageHandler {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            compression_service: ImageCompressionService::new(),
            cors_config: config.cors.clone(),
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
            match self.process_http_request_body(req).await {
                Ok(response_body) => Ok(self.create_cors_response(
                    StatusCode::OK,
                    Body::from(response_body),
                    origin.as_deref(),
                )),
                Err(error_body) => Ok(self.create_cors_response(
                    StatusCode::BAD_REQUEST,
                    Body::from(error_body),
                    origin.as_deref(),
                )),
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
                    Body::from(error_body),
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

        let body = event
            .payload
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match self.process_request_body(body).await {
            Ok(response_json) => Ok(json!({
                "statusCode": 200,
                "headers": self.get_cors_headers(origin),
                "body": response_json
            })),
            Err(error_json) => Ok(json!({
                "statusCode": 400,
                "headers": self.get_cors_headers(origin),
                "body": error_json
            })),
        }
    }

    async fn process_http_request_body(&self, req: Request<Body>) -> Result<String, String> {
        let body_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .map_err(|_| r#"{"error":"Error reading request body"}"#.to_string())?;

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
            .map_err(|_| r#"{"error":"Error reading image data"}"#.to_string())?;

        if image_bytes.is_empty() {
            return Err(r#"{"error":"No image data provided"}"#.to_string());
        }

        let image_base64 = general_purpose::STANDARD.encode(&image_bytes);

        let request = OptimizeRequest {
            image_data: image_base64,
            quality,
            format,
            progressive,
            aggressive,
        };

        let result = self.compression_service.optimize_image(request).await?;
        let response = self.compression_service.create_response(result);

        serde_json::to_string(&response)
            .map_err(|_| r#"{"error":"Error serializing response"}"#.to_string())
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

        let result = self.compression_service.optimize_image(request).await?;
        let response = self.compression_service.create_response(result);

        serde_json::to_string(&response)
            .map_err(|_| r#"{"error":"Error serializing response"}"#.to_string())
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
}
