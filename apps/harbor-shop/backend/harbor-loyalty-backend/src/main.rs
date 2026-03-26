use std::{env, sync::Arc};

use axum::{
    Router,
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use harbor_loyalty_backend::{
    CrmContactUpdate, LoyaltyPreviewRequest, compute_loyalty_preview, health_response,
    route_crm_contact, service_overview, webhook_secret_matches,
};

#[derive(Clone)]
struct AppState {
    brand: String,
    webhook_secret: String,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: &'static str,
    detail: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind = env::var("HARBOR_BACKEND_BIND").unwrap_or_else(|_| "0.0.0.0:8081".to_string());
    let state = Arc::new(AppState {
        brand: env::var("HARBOR_BACKEND_BRAND").unwrap_or_else(|_| "Harbor Shop".to_string()),
        webhook_secret: env::var("HARBOR_BACKEND_WEBHOOK_SECRET")
            .unwrap_or_else(|_| "harbor-backend-dev-secret".to_string()),
    });

    let app = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/loyalty/preview", post(loyalty_preview))
        .route("/webhooks/crm/contact-updated", post(contact_updated))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("harbor-loyalty-backend listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(service_overview(&state.brand))
}

async fn health() -> impl IntoResponse {
    Json(health_response())
}

async fn loyalty_preview(
    Json(request): Json<LoyaltyPreviewRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if request.customer_email.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request",
                detail: "customer_email must not be empty".to_string(),
            }),
        ));
    }

    if request.subtotal_gbp.is_sign_negative() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request",
                detail: "subtotal_gbp must be zero or greater".to_string(),
            }),
        ));
    }

    Ok((StatusCode::OK, Json(compute_loyalty_preview(&request))))
}

async fn contact_updated(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(update): Json<CrmContactUpdate>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let provided_secret = headers
        .get("x-harbor-backend-secret")
        .and_then(|value| value.to_str().ok());

    if !webhook_secret_matches(&state.webhook_secret, provided_secret) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "webhook_verification_failed",
                detail: "x-harbor-backend-secret did not match the configured backend secret"
                    .to_string(),
            }),
        ));
    }

    Ok((StatusCode::ACCEPTED, Json(route_crm_contact(&update))))
}
