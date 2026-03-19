use super::*;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::observability::serve_diagnostics_probe;

pub(crate) fn privileged_router() -> Router<Arc<RuntimeServerState>> {
    Router::new()
        .route("/diagnostics", get(serve_diagnostics_probe))
        .route_layer(middleware::from_fn(require_diagnostics_access))
}

async fn require_diagnostics_access(
    State(state): State<Arc<RuntimeServerState>>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match authorize_diagnostics_access(&state, &request).await {
        Ok(()) => next.run(request).await,
        Err(error) => error_response(error),
    }
}

async fn authorize_diagnostics_access(
    state: &RuntimeServerState,
    request: &Request<Body>,
) -> Result<(), RuntimeServerError> {
    let live_request = LiveHttpRequest::from_request(
        request,
        &state.plan.browser,
        &state.plan.config.server,
        None,
    )?;
    let request = live_request.into_request_input()?;
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let resolved = {
        let mut browser = state
            .browser
            .lock()
            .expect("runtime browser mutex poisoned");
        browser
            .resolve_request(&request, &state.cookie_secret, now)
            .map_err(RequestExecutionError::from_browser_error)?
    };

    let Some(principal_id) = resolved.principal_id.as_deref() else {
        return Err(RuntimeServerError::Execution(
            RequestExecutionError::SessionRequired {
                route: "diagnostics".to_string(),
            },
        ));
    };

    let subject =
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(principal_id.to_string()));
    let object = davenda_auth::Entity::admin_module(state.plan.config.app.name.clone());
    let allowed = state
        .route_authorizer
        .check_capability(&subject, davenda_auth::Capability::AdminAuditRead, &object)
        .await?;

    if !allowed {
        return Err(RuntimeServerError::Execution(
            RequestExecutionError::CapabilityRequired {
                route: "diagnostics".to_string(),
                capability: davenda_auth::Capability::AdminAuditRead,
            },
        ));
    }

    Ok(())
}
