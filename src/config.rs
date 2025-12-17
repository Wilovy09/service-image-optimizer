use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub compression: CompressionConfig,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone)]
pub struct CompressionConfig {
    pub max_image_size: usize,
    pub default_quality: u8,
    pub aggressive_quality: u8,
    pub timeout_seconds: u64,
}

impl AppConfig {
    pub fn from_env() -> Self {
        dotenv::dotenv().ok();

        Self {
            server: ServerConfig {
                host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: env::var("PORT")
                    .unwrap_or_else(|_| "8080".to_string())
                    .parse()
                    .unwrap_or(8080),
                timeout_seconds: env::var("SERVER_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .unwrap_or(30),
            },
            compression: CompressionConfig {
                max_image_size: env::var("MAX_IMAGE_SIZE")
                    .unwrap_or_else(|_| "52428800".to_string()) // 50MB
                    .parse()
                    .unwrap_or(50 * 1024 * 1024),
                default_quality: env::var("DEFAULT_QUALITY")
                    .unwrap_or_else(|_| "75".to_string())
                    .parse()
                    .unwrap_or(75),
                aggressive_quality: env::var("AGGRESSIVE_QUALITY")
                    .unwrap_or_else(|_| "60".to_string())
                    .parse()
                    .unwrap_or(60),
                timeout_seconds: env::var("COMPRESSION_TIMEOUT")
                    .unwrap_or_else(|_| "10".to_string())
                    .parse()
                    .unwrap_or(10),
            },
        }
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn is_running_on_lambda(&self) -> bool {
        env::var("AWS_LAMBDA_RUNTIME_API").is_ok()
    }
}
