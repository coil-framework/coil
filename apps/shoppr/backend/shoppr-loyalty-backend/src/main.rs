use std::env;

use shoppr_loyalty_backend::{BackendConfig, build_router};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // This binary is the optional sidecar wrapper around the linked Shoppr customer backend crate.
    let bind = env::var("SHOPPR_BACKEND_BIND").unwrap_or_else(|_| "0.0.0.0:8081".to_string());
    let config = BackendConfig {
        brand: env::var("SHOPPR_BACKEND_BRAND").unwrap_or_else(|_| "Shoppr".to_string()),
        webhook_secret: env::var("SHOPPR_BACKEND_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "shoppr-backend-dev-secret".to_string()),
    };
    let app = build_router(config);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("shoppr-loyalty-backend listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}
