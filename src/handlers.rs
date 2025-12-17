use crate::models::*;
use crate::services::ImageCompressionService;
use hyper::{Body, Method, Request, Response, StatusCode};
use lambda_runtime::{Error, LambdaEvent};
use serde_json::{Value, json};
use std::convert::Infallible;

pub struct ImageHandler {
    compression_service: ImageCompressionService,
}

impl ImageHandler {
    pub fn new() -> Self {
        Self {
            compression_service: ImageCompressionService::new(),
        }
    }

    pub async fn handle_http_request(
        &self,
        req: Request<Body>,
    ) -> Result<Response<Body>, Infallible> {
        if req.method() == Method::OPTIONS {
            return Ok(self.create_cors_response(StatusCode::OK, Body::empty()));
        }

        if req.method() == Method::POST && req.uri().path() == "/optimize" {
            match self.process_http_request(req).await {
                Ok(response_body) => {
                    Ok(self.create_cors_response(StatusCode::OK, Body::from(response_body)))
                }
                Err(error_body) => {
                    Ok(self.create_cors_response(StatusCode::BAD_REQUEST, Body::from(error_body)))
                }
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

        let body = event
            .payload
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match self.process_request_body(body).await {
            Ok(response_json) => Ok(json!({
                "statusCode": 200,
                "headers": self.get_cors_headers(),
                "body": response_json
            })),
            Err(error_json) => Ok(json!({
                "statusCode": 400,
                "headers": self.get_cors_headers(),
                "body": error_json
            })),
        }
    }

    async fn process_http_request(&self, req: Request<Body>) -> Result<String, String> {
        let body_bytes = hyper::body::to_bytes(req.into_body())
            .await
            .map_err(|_| r#"{"error":"Error reading request body"}"#.to_string())?;

        let body_str = String::from_utf8_lossy(&body_bytes);
        self.process_request_body(&body_str).await
    }

    async fn process_request_body(&self, body_str: &str) -> Result<String, String> {
        let request = if let Ok(req) = serde_json::from_str::<OptimizeRequest>(body_str) {
            req
        } else {
            OptimizeRequest {
                image_data: body_str.to_string(),
                quality: 60, // Aggressive quality for max compression
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

    fn create_cors_response(&self, status: StatusCode, body: Body) -> Response<Body> {
        Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "POST, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type")
            .body(body)
            .unwrap()
    }

    fn get_cors_headers(&self) -> serde_json::Map<String, Value> {
        let mut headers = serde_json::Map::new();
        headers.insert("Content-Type".to_string(), json!("application/json"));
        headers.insert("Access-Control-Allow-Origin".to_string(), json!("*"));
        headers.insert(
            "Access-Control-Allow-Methods".to_string(),
            json!("POST, OPTIONS"),
        );
        headers.insert(
            "Access-Control-Allow-Headers".to_string(),
            json!("Content-Type"),
        );
        headers
    }
}
