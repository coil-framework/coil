//! Optional Axum sidecar adapter for the Shoppr linked customer backend example.
//!
//! The primary path for this crate is linking `plugin()` into a customer-owned binary. This file
//! exists only for cases where the same Rust rules genuinely need a separate HTTP/process
//! boundary.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Json as ExtractJson, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};

use crate::{
    CrmContactUpdate, ShopprCustomerBackend, LoyaltyPreviewRequest, OrderReviewRequest,
    health_response, plugin, service_overview, webhook_secret_matches,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendConfig {
    pub brand: String,
    pub webhook_secret: String,
}

#[derive(Clone)]
struct AppState {
    brand: String,
    webhook_secret: String,
    backend: ShopprCustomerBackend,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ErrorResponse {
    error: String,
    detail: String,
}

pub fn build_router(config: BackendConfig) -> Router {
    let state = Arc::new(AppState {
        brand: config.brand,
        webhook_secret: config.webhook_secret,
        backend: plugin(),
    });

    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .route("/api/loyalty/preview", post(loyalty_preview))
        .route("/api/orders/review", post(order_review))
        .route("/webhooks/crm/contact-updated", post(contact_updated))
        .with_state(state)
}

async fn index(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(service_overview(&state.brand))
}

async fn health() -> impl IntoResponse {
    Json(health_response())
}

async fn loyalty_preview(
    State(state): State<Arc<AppState>>,
    ExtractJson(request): ExtractJson<LoyaltyPreviewRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if request.customer_email.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                detail: "customer_email must not be empty".to_string(),
            }),
        ));
    }

    if request.subtotal_gbp.is_sign_negative() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                detail: "subtotal_gbp must be zero or greater".to_string(),
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(state.backend.preview_loyalty(&request)),
    ))
}

async fn order_review(
    State(state): State<Arc<AppState>>,
    ExtractJson(request): ExtractJson<OrderReviewRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if request.customer_email.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                detail: "customer_email must not be empty".to_string(),
            }),
        ));
    }

    if request.subtotal_gbp.is_sign_negative() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                detail: "subtotal_gbp must be zero or greater".to_string(),
            }),
        ));
    }

    if request.shipping_country.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid_request".to_string(),
                detail: "shipping_country must not be empty".to_string(),
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(state.backend.review_checkout_order(&request)),
    ))
}

async fn contact_updated(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    ExtractJson(update): ExtractJson<CrmContactUpdate>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    let provided_secret = headers
        .get("x-harbor-backend-secret")
        .and_then(|value| value.to_str().ok());

    if !webhook_secret_matches(&state.webhook_secret, provided_secret) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "webhook_verification_failed".to_string(),
                detail: "x-harbor-backend-secret did not match the configured backend secret"
                    .to_string(),
            }),
        ));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(state.backend.route_crm_contact_update(&update)),
    ))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde::de::DeserializeOwned;
    use tower::util::ServiceExt;

    use super::*;
    use crate::{CrmContactRoute, HealthResponse, OrderReviewResponse, ServiceOverview};

    fn config() -> BackendConfig {
        BackendConfig {
            brand: "Shoppr".to_string(),
            webhook_secret: "harbor-backend-dev-secret".to_string(),
        }
    }

    async fn response_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn root_route_reports_brand_and_endpoints() {
        let response = build_router(config())
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let overview: ServiceOverview = response_json(response).await;
        assert_eq!(overview.brand, "Shoppr");
        assert!(
            overview
                .endpoints
                .contains(&"POST /api/loyalty/preview".to_string())
        );
        assert!(
            overview
                .endpoints
                .contains(&"POST /api/orders/review".to_string())
        );
        assert!(
            overview
                .endpoints
                .contains(&"POST /webhooks/crm/contact-updated".to_string())
        );
    }

    #[tokio::test]
    async fn health_route_returns_ok() {
        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let health: HealthResponse = response_json(response).await;
        assert_eq!(health.status, "ok");
    }

    #[tokio::test]
    async fn loyalty_preview_rejects_empty_email() {
        let request = serde_json::json!({
            "customer_email": "",
            "membership_tier": "standard",
            "subtotal_gbp": 42.0,
            "cart_skus": ["harbor-cap"],
            "collection_handle": "featured"
        });

        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loyalty/preview")
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = response_json(response).await;
        assert_eq!(error.error, "invalid_request");
        assert!(error.detail.contains("customer_email"));
    }

    #[tokio::test]
    async fn crm_webhook_rejects_missing_secret() {
        let request = serde_json::json!({
            "customer_email": "member@harbor.test",
            "membership_tier": "standard",
            "lifecycle_stage": "winback",
            "last_order_total_gbp": 42.0
        });

        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/crm/contact-updated")
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: ErrorResponse = response_json(response).await;
        assert_eq!(error.error, "webhook_verification_failed");
    }

    #[tokio::test]
    async fn order_review_rejects_missing_shipping_country() {
        let request = serde_json::json!({
            "customer_email": "member@harbor.test",
            "membership_tier": "standard",
            "subtotal_gbp": 64.0,
            "cart_skus": ["harbor-cap"],
            "shipping_country": "",
            "expedited_requested": false
        });

        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/orders/review")
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: ErrorResponse = response_json(response).await;
        assert_eq!(error.error, "invalid_request");
        assert!(error.detail.contains("shipping_country"));
    }

    #[tokio::test]
    async fn order_review_returns_customer_specific_fulfilment_decision() {
        let request = serde_json::json!({
            "customer_email": "captain@harbor.test",
            "membership_tier": "gold",
            "subtotal_gbp": 240.0,
            "cart_skus": ["cellar-tour-pass"],
            "shipping_country": "GB",
            "expedited_requested": true
        });

        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/orders/review")
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let review: OrderReviewResponse = response_json(response).await;
        assert!(review.review_required);
        assert_eq!(review.assigned_queue, "ops-manual-review");
        assert!(review.tags.contains(&"ops:manual-review".to_string()));
    }

    #[tokio::test]
    async fn crm_webhook_routes_known_member_updates() {
        let request = serde_json::json!({
            "customer_email": "captain@harbor.test",
            "membership_tier": "gold",
            "lifecycle_stage": "retained",
            "last_order_total_gbp": 175.0
        });

        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/crm/contact-updated")
                    .header("content-type", "application/json")
                    .header("x-harbor-backend-secret", "harbor-backend-dev-secret")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let route: CrmContactRoute = response_json(response).await;
        assert_eq!(route.segment, "harbor-vip");
        assert!(route.follow_up_required);
    }

    #[tokio::test]
    async fn loyalty_preview_returns_customer_specific_rules() {
        let request = serde_json::json!({
            "customer_email": "member@harbor.test",
            "membership_tier": "standard",
            "subtotal_gbp": 64.0,
            "cart_skus": ["tasting-pass"],
            "collection_handle": "events"
        });

        let response = build_router(config())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/loyalty/preview")
                    .header("content-type", "application/json")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let preview: crate::LoyaltyPreviewResponse = response_json(response).await;
        assert_eq!(preview.segment, "harbor-member");
        assert!(preview.priority_fulfilment);
        assert_eq!(preview.discount_bps, 250);
    }
}
