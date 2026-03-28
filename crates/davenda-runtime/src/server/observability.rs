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
    let readiness = live_readiness_report(&state).await;
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
            "readiness": health_report_json(&readiness),
            "maintenance": maintenance_mode_json(&state.plan.observability.maintenance),
        }),
    )
}

pub(crate) async fn serve_readiness_probe(
    State(state): State<Arc<RuntimeServerState>>,
) -> Response<Body> {
    let readiness_report = live_readiness_report(&state).await;
    let readiness = readiness_report.overall_status();
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
            "readiness": health_report_json(&readiness_report),
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
            let reading = telemetry.metric_reading(metric.name.as_str());
            json!({
                "name": metric.name.to_string(),
                "kind": metric_kind_string(metric.kind),
                "unit": metric_unit_string(metric.unit),
                "dimensions": metric
                    .dimensions
                    .iter()
                    .map(|dimension| dimension.to_string())
                    .collect::<Vec<_>>(),
                "reading": metric_reading_json(reading),
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

async fn live_readiness_report(state: &RuntimeServerState) -> davenda_observability::HealthReport {
    let mut readiness = state.plan.observability.readiness.clone();

    let database_status = if readiness
        .dependency(davenda_observability::DependencyKind::Database)
        .is_some()
    {
        Some(live_database_status(state).await)
    } else {
        None
    };

    if let Some(status) = database_status {
        readiness.set_dependency_status(davenda_observability::DependencyKind::Database, status);
    }

    if readiness
        .dependency(davenda_observability::DependencyKind::Queue)
        .is_some()
    {
        let status = match database_status {
            Some(davenda_observability::DependencyStatus::Healthy) => {
                davenda_observability::DependencyStatus::Healthy
            }
            Some(status) => status,
            None => davenda_observability::DependencyStatus::Unknown,
        };
        readiness.set_dependency_status(davenda_observability::DependencyKind::Queue, status);
    }

    if readiness
        .dependency(davenda_observability::DependencyKind::DistributedCache)
        .is_some()
    {
        let status = if state.backends.distributed_cache.is_some() {
            davenda_observability::DependencyStatus::Healthy
        } else {
            davenda_observability::DependencyStatus::Unhealthy
        };
        readiness.set_dependency_status(
            davenda_observability::DependencyKind::DistributedCache,
            status,
        );
    }

    if readiness
        .dependency(davenda_observability::DependencyKind::ObjectStore)
        .is_some()
    {
        let status = if state.backends.object_store.is_some() {
            davenda_observability::DependencyStatus::Healthy
        } else {
            davenda_observability::DependencyStatus::Unhealthy
        };
        readiness.set_dependency_status(davenda_observability::DependencyKind::ObjectStore, status);
    }

    if readiness
        .dependency(davenda_observability::DependencyKind::Secrets)
        .is_some()
    {
        readiness.set_dependency_status(
            davenda_observability::DependencyKind::Secrets,
            davenda_observability::DependencyStatus::Healthy,
        );
    }

    if readiness
        .dependency(davenda_observability::DependencyKind::Tls)
        .is_some()
    {
        readiness.set_dependency_status(
            davenda_observability::DependencyKind::Tls,
            davenda_observability::DependencyStatus::Healthy,
        );
    }

    if readiness
        .dependency(davenda_observability::DependencyKind::ExtensionRegistry)
        .is_some()
    {
        readiness.set_dependency_status(
            davenda_observability::DependencyKind::ExtensionRegistry,
            davenda_observability::DependencyStatus::Healthy,
        );
    }

    readiness
}

async fn live_database_status(
    state: &RuntimeServerState,
) -> davenda_observability::DependencyStatus {
    if state.plan.data.driver != davenda_config::DatabaseDriver::Postgres {
        return davenda_observability::DependencyStatus::Healthy;
    }

    let Some(connection_url) = state.backends.database.url.clone() else {
        return davenda_observability::DependencyStatus::Unhealthy;
    };
    let client = match state.plan.data.with_resolved_connection_url(connection_url).connect_lazy_postgres() {
        Ok(client) => client,
        Err(_) => return davenda_observability::DependencyStatus::Unhealthy,
    };

    match client.ping().await {
        Ok(()) => davenda_observability::DependencyStatus::Healthy,
        Err(_) => davenda_observability::DependencyStatus::Unhealthy,
    }
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

fn metric_reading_json(reading: Option<davenda_observability::MetricReading>) -> serde_json::Value {
    match reading {
        Some(davenda_observability::MetricReading::Counter(value)) => json!({
            "counter": value,
        }),
        Some(davenda_observability::MetricReading::Gauge(value)) => json!({
            "gauge": value,
        }),
        Some(davenda_observability::MetricReading::Histogram(value)) => json!({
            "histogram": {
                "samples": value.samples,
                "last": value.last,
                "max": value.max,
            }
        }),
        None => serde_json::Value::Null,
    }
}
