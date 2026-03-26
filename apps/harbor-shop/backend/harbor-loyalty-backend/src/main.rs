use std::env;

use harbor_loyalty_backend::{BackendConfig, build_router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind = env::var("HARBOR_BACKEND_BIND").unwrap_or_else(|_| "0.0.0.0:8081".to_string());
    let config = BackendConfig {
        brand: env::var("HARBOR_BACKEND_BRAND").unwrap_or_else(|_| "Harbor Shop".to_string()),
        webhook_secret: env::var("HARBOR_BACKEND_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "harbor-backend-dev-secret".to_string()),
    };
    let app = build_router(config);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("harbor-loyalty-backend listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
