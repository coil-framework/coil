use super::*;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header::CONTENT_TYPE};
use axum::response::Response;
use axum::routing::{any, get};
use serde_json::json;
use std::sync::Arc;

pub(crate) fn public_router() -> Router<Arc<RuntimeServerState>> {
    Router::new()
        .route("/health", any(serve_health_probe))
        .route("/ready", any(serve_readiness_probe))
        .route("/readiness", any(serve_readiness_probe))
        .route("/metrics", get(serve_metrics_probe))
}

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

pub(super) fn observability_response(
    status: StatusCode,
    value: serde_json::Value,
) -> Response<Body> {
    let mut response = Response::new(Body::from(value.to_string()));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

pub(super) fn health_report_json(
    report: &davenda_observability::HealthReport,
) -> serde_json::Value {
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

pub(super) fn maintenance_mode_json(
    mode: &davenda_observability::MaintenanceMode,
) -> serde_json::Value {
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
