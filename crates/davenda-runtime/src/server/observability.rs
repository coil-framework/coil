use super::*;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header::CONTENT_TYPE};
use axum::response::Response;
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) async fn serve_health_probe(
    State(state): State<Arc<RuntimeServerState>>,
) -> Response<Body> {
    let liveness = state.plan.observability.liveness.overall_status();
    let status = match liveness {
        davenda_observability::DependencyStatus::Healthy => StatusCode::OK,
        davenda_observability::DependencyStatus::Degraded
        | davenda_observability::DependencyStatus::Unhealthy
        | davenda_observability::DependencyStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
    };

    observability_response(
        status,
        json!({
            "status": health_status_string(liveness),
            "liveness": health_report_json(&state.plan.observability.liveness),
            "readiness": health_report_json(&state.plan.observability.readiness),
            "maintenance": maintenance_mode_json(&state.plan.observability.maintenance),
        }),
    )
}

pub(crate) async fn serve_readiness_probe(
    State(state): State<Arc<RuntimeServerState>>,
) -> Response<Body> {
    let readiness = state.plan.observability.readiness.overall_status();
    let status = match readiness {
        davenda_observability::DependencyStatus::Healthy => StatusCode::OK,
        davenda_observability::DependencyStatus::Degraded
        | davenda_observability::DependencyStatus::Unhealthy
        | davenda_observability::DependencyStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
    };

    observability_response(
        status,
        json!({
            "status": health_status_string(readiness),
            "readiness": health_report_json(&state.plan.observability.readiness),
        }),
    )
}

pub(crate) async fn serve_metrics_probe(
    State(state): State<Arc<RuntimeServerState>>,
) -> Response<Body> {
    let telemetry = &state.plan.observability.telemetry;
    let metrics = telemetry
        .metrics
        .values()
        .map(|metric| {
            json!({
                "name": metric.name.to_string(),
                "kind": metric_kind_string(metric.kind),
                "unit": metric_unit_string(metric.unit),
                "dimensions": metric
                    .dimensions
                    .iter()
                    .map(|dimension| dimension.to_string())
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    observability_response(
        StatusCode::OK,
        json!({
            "metrics_enabled": telemetry.metrics_enabled,
            "trace": {
                "enabled": telemetry.trace.enabled,
                "sample_permyriad": telemetry.trace.sample_permyriad,
            },
            "metrics": metrics,
        }),
    )
}

pub(crate) async fn serve_diagnostics_probe(
    State(state): State<Arc<RuntimeServerState>>,
    request: axum::http::Request<Body>,
) -> Response<Body> {
    match diagnose_request(state, request).await {
        Ok(response) => response,
        Err(error) => error_response(error),
    }
}

async fn diagnose_request(
    state: Arc<RuntimeServerState>,
    request: axum::http::Request<Body>,
) -> Result<Response<Body>, RuntimeServerError> {
    let live_request = LiveHttpRequest::from_request(
        &request,
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
        .check_capability(
            &subject,
            davenda_auth::Capability::AdminAuditRead,
            &object,
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

    let backends = &state.backends;
    let metadata_audit = match state.wasm_host.metadata_audit_snapshot(25) {
        Ok(snapshot) => json!({
            "backend": snapshot.backend.as_str(),
            "shared_namespace": state.plan.shared_backend_namespace(),
            "location": snapshot.location,
            "path": snapshot.path.map(|path| path.display().to_string()),
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

    let mut response = observability_response(
        StatusCode::OK,
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

    for cookie in resolved.response_cookies {
        response.headers_mut().append(
            axum::http::header::SET_COOKIE,
            HeaderValue::from_str(&cookie).map_err(|_| RuntimeServerError::InvalidHeaderValue {
                header: "set-cookie",
            })?,
        );
    }

    Ok(response)
}

fn observability_response(status: StatusCode, value: serde_json::Value) -> Response<Body> {
    let mut response = Response::new(Body::from(value.to_string()));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

fn health_report_json(report: &davenda_observability::HealthReport) -> serde_json::Value {
    json!({
        "kind": report.kind.to_string(),
        "status": health_status_string(report.overall_status()),
        "dependencies": report.dependencies.iter().map(|dependency| json!({
            "kind": dependency.kind.to_string(),
            "required": dependency.required,
            "status": health_status_string(dependency.status),
        })).collect::<Vec<_>>(),
    })
}

fn maintenance_mode_json(mode: &davenda_observability::MaintenanceMode) -> serde_json::Value {
    json!({
        "enabled": mode.enabled,
        "audience": match &mode.audience {
            davenda_observability::MaintenanceAudience::Deployment => "deployment",
            davenda_observability::MaintenanceAudience::CustomerApp(app_id) => app_id.as_str(),
        },
        "impact": match mode.impact {
            davenda_observability::MaintenanceImpact::AllTraffic => "all_traffic",
            davenda_observability::MaintenanceImpact::MutatingTrafficOnly => "mutating_traffic_only",
        },
        "allowed_background_work": mode
            .allowed_background_work
            .iter()
            .map(|work| format!("{work:?}"))
            .collect::<Vec<_>>(),
    })
}

fn health_status_string(status: davenda_observability::DependencyStatus) -> &'static str {
    match status {
        davenda_observability::DependencyStatus::Healthy => "healthy",
        davenda_observability::DependencyStatus::Degraded => "degraded",
        davenda_observability::DependencyStatus::Unhealthy => "unhealthy",
        davenda_observability::DependencyStatus::Unknown => "unknown",
    }
}

fn metric_kind_string(kind: davenda_observability::MetricKind) -> &'static str {
    match kind {
        davenda_observability::MetricKind::Counter => "counter",
        davenda_observability::MetricKind::Gauge => "gauge",
        davenda_observability::MetricKind::Histogram => "histogram",
    }
}

fn metric_unit_string(unit: davenda_observability::MetricUnit) -> &'static str {
    match unit {
        davenda_observability::MetricUnit::Count => "count",
        davenda_observability::MetricUnit::Milliseconds => "milliseconds",
        davenda_observability::MetricUnit::Bytes => "bytes",
        davenda_observability::MetricUnit::Ratio => "ratio",
    }
}
