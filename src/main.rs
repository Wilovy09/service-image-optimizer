use hyper::Server;
use hyper::service::{make_service_fn, service_fn as hyper_service_fn};
use lambda_runtime::service_fn;
use std::convert::Infallible;
mod config;
mod handlers;
mod models;
mod services;
mod utils;
use config::AppConfig;
use handlers::ImageHandler;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();

    if config.is_running_on_lambda() {
        println!("🔺 Starting Lambda function");
        let config_clone = config.clone();

        lambda_runtime::run(service_fn(move |event| {
            let handler = ImageHandler::new(&config_clone);
            async move { handler.handle_lambda_event(event).await }
        }))
        .await?;
    } else {
        println!("💻 Starting local server at {}", config.server_address());
        println!("- Use POST /optimize with JSON or multipart/form-data");
        println!("- Use POST /resize with multipart/form-data");
        println!(
            "- Max image size: {} MB",
            config.compression.max_image_size / (1024 * 1024) // 50MB
        );
        println!("- Default quality: {}", config.compression.default_quality);
        println!(
            "- Aggressive quality: {}",
            config.compression.aggressive_quality
        );

        let addr = config.server_address().parse()?;
        let config_for_service = config.clone();

        let make_svc = make_service_fn(move |_conn| {
            let config_clone = config_for_service.clone();
            async move {
                Ok::<_, Infallible>(hyper_service_fn(move |req| {
                    let handler = ImageHandler::new(&config_clone);
                    async move { handler.handle_http_request(req).await }
                }))
            }
        });

        let server = Server::bind(&addr).serve(make_svc);

        println!("- Server running on http://{}", addr);

        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    }

    Ok(())
}
