use super::observability::{health_report_json, maintenance_mode_json, observability_response};
use super::*;
use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::extract::State;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn privileged_router(state: Arc<RuntimeServerState>) -> Router<Arc<RuntimeServerState>> {
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

pub(crate) async fn serve_diagnostics_probe(
    State(state): State<Arc<RuntimeServerState>>,
) -> Response<Body> {
    let backends = &state.backends;
    let metadata_audit = match state.wasm_host.metadata_audit_snapshot(25) {
        Ok(snapshot) => json!({
            "backend": snapshot.backend.as_str(),
            "shared_namespace": state.plan.shared_backend_namespace(),
            "location": snapshot.location,
            "path": snapshot.path.as_ref().map(|path| path.display().to_string()),
            "entry_count": snapshot.entry_count,
            "recent_records": snapshot
                .recent_records
                .into_iter()
                .map(|record| json!({
                    "id": record.id,
                    "recorded_at_unix_seconds": record.recorded_at_unix_seconds,
                    "kind": record.kind,
                    "app_id": record.app_id,
                    "trace_id": record.trace_id,
                    "request_id": record.request_id,
                    "principal_kind": record.principal_kind,
                    "principal_id": record.principal_id,
                }))
                .collect::<Vec<_>>(),
        }),
        Err(error) => json!({
            "backend": state.wasm_host.metadata_audit_backend_kind(),
            "shared_namespace": state.plan.shared_backend_namespace(),
            "location": state.wasm_host.metadata_audit_location(),
            "error": error,
        }),
    };

    let response = observability_response(
        axum::http::StatusCode::OK,
        json!({
            "customer_app": state.plan.config.app.name,
            "observability": {
                "health": {
                    "liveness": health_report_json(&state.plan.observability.liveness),
                    "readiness": health_report_json(&state.plan.observability.readiness),
                },
                "maintenance": maintenance_mode_json(&state.plan.observability.maintenance),
                "telemetry": {
                    "metrics_enabled": state.plan.observability.telemetry.metrics_enabled,
                    "trace": {
                        "enabled": state.plan.observability.telemetry.trace.enabled,
                        "sample_permyriad": state.plan.observability.telemetry.trace.sample_permyriad,
                    },
                },
            },
            "backends": {
                "database": {
                    "driver": format!("{:?}", backends.database.driver),
                    "shared": true,
                },
                "distributed_cache": backends
                    .distributed_cache
                    .as_ref()
                    .map(|backend| json!({
                        "backend": format!("{:?}", backend.backend),
                        "purpose": backend.purpose,
                    })),
                "jobs": {
                    "backend": format!("{:?}", backends.jobs.backend),
                    "shared": backends.jobs.shared,
                },
                "session_store": backends
                    .session_store
                    .as_ref()
                    .map(|backend| json!({
                        "kind": format!("{:?}", backend.kind),
                        "shared": backend.shared,
                    })),
                "object_store": backends
                    .object_store
                    .as_ref()
                    .map(|backend| json!({
                        "kind": format!("{:?}", backend.kind),
                        "credential_reference": backend.credential_reference,
                        "local_root": backend.local_root,
                    })),
            },
            "metadata": metadata_audit,
        }),
    );
    response
}
