use super::*;
use axum::Router;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::routing::get;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::observability::serve_diagnostics_probe;

pub(crate) fn privileged_router(
    state: Arc<RuntimeServerState>,
) -> Router<Arc<RuntimeServerState>> {
    let auth_state = state.clone();
    Router::new()
        .route("/diagnostics", get(serve_diagnostics_probe))
        .layer(middleware::from_fn(move |request: Request, next: Next| {
            let state = auth_state.clone();
            async move {
                let authorization = match prepare_diagnostics_access(&state, &request) {
                    Ok(check) => authorize_diagnostics_access(&state, check).await,
                    Err(error) => Err(error),
                };
                match authorization {
                    Ok(()) => next.run(request).await,
                    Err(error) => error_response(error),
                }
            }
        }))
}

struct DiagnosticsAccessCheck {
    subject: davenda_auth::DefaultSubject,
    object: davenda_auth::Entity,
}

fn prepare_diagnostics_access(
    state: &RuntimeServerState,
    request: &Request,
) -> Result<DiagnosticsAccessCheck, RuntimeServerError> {
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

    Ok(DiagnosticsAccessCheck {
        subject: davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(
            principal_id.to_string(),
        )),
        object: davenda_auth::Entity::admin_module(state.plan.config.app.name.clone()),
    })
}

async fn authorize_diagnostics_access(
    state: &RuntimeServerState,
    check: DiagnosticsAccessCheck,
) -> Result<(), RuntimeServerError> {
    let allowed = state
        .route_authorizer
        .check_capability(
            &check.subject,
            davenda_auth::Capability::AdminAuditRead,
            &check.object,
        )
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
