use super::*;
use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, StatusCode, header::CONTENT_TYPE};
use axum::response::Response;
use axum::routing::{any, get};
use coil_cache::{CacheBackendKind, CacheMetrics};
use coil_jobs::JobsCoordinatorSnapshot;
use serde_json::json;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
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
        coil_observability::DependencyStatus::Healthy => StatusCode::OK,
        coil_observability::DependencyStatus::Degraded
        | coil_observability::DependencyStatus::Unhealthy
        | coil_observability::DependencyStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
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
        coil_observability::DependencyStatus::Healthy => StatusCode::OK,
        coil_observability::DependencyStatus::Degraded
        | coil_observability::DependencyStatus::Unhealthy
        | coil_observability::DependencyStatus::Unknown => StatusCode::SERVICE_UNAVAILABLE,
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
    let live_jobs_snapshot = live_jobs_snapshot(state.as_ref());
    let live_cache_metrics = live_cache_metrics(state.as_ref());

    text_response(
        StatusCode::OK,
        "text/plain; version=0.0.4; charset=utf-8",
        prometheus_metrics_body(
            telemetry,
            live_jobs_snapshot.as_ref(),
            live_cache_metrics.as_ref(),
        ),
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
    report: &coil_observability::HealthReport,
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

async fn live_readiness_report(state: &RuntimeServerState) -> coil_observability::HealthReport {
    let mut readiness = state.plan.observability.readiness.clone();

    let database_status = if readiness
        .dependency(coil_observability::DependencyKind::Database)
        .is_some()
    {
        Some(live_database_status(state).await)
    } else {
        None
    };

    if let Some(status) = database_status {
        readiness.set_dependency_status(coil_observability::DependencyKind::Database, status);
    }

    if readiness
        .dependency(coil_observability::DependencyKind::Queue)
        .is_some()
    {
        let status = live_queue_status(state);
        readiness.set_dependency_status(coil_observability::DependencyKind::Queue, status);
    }

    if readiness
        .dependency(coil_observability::DependencyKind::DistributedCache)
        .is_some()
    {
        let status = live_distributed_cache_status(state);
        readiness.set_dependency_status(
            coil_observability::DependencyKind::DistributedCache,
            status,
        );
    }

    if readiness
        .dependency(coil_observability::DependencyKind::ObjectStore)
        .is_some()
    {
        let status = live_object_store_status(state);
        readiness.set_dependency_status(coil_observability::DependencyKind::ObjectStore, status);
    }

    if readiness
        .dependency(coil_observability::DependencyKind::Secrets)
        .is_some()
    {
        readiness.set_dependency_status(
            coil_observability::DependencyKind::Secrets,
            coil_observability::DependencyStatus::Healthy,
        );
    }

    if readiness
        .dependency(coil_observability::DependencyKind::Tls)
        .is_some()
    {
        readiness.set_dependency_status(
            coil_observability::DependencyKind::Tls,
            coil_observability::DependencyStatus::Healthy,
        );
    }

    if readiness
        .dependency(coil_observability::DependencyKind::ExtensionRegistry)
        .is_some()
    {
        readiness.set_dependency_status(
            coil_observability::DependencyKind::ExtensionRegistry,
            coil_observability::DependencyStatus::Healthy,
        );
    }

    readiness
}

async fn live_database_status(
    state: &RuntimeServerState,
) -> coil_observability::DependencyStatus {
    if state.plan.data.driver != coil_config::DatabaseDriver::Postgres {
        return coil_observability::DependencyStatus::Healthy;
    }

    let Some(connection_url) = state.backends.database.url.clone() else {
        return coil_observability::DependencyStatus::Unhealthy;
    };
    let client = match state.plan.data.with_resolved_connection_url(connection_url).connect_lazy_postgres() {
        Ok(client) => client,
        Err(_) => return coil_observability::DependencyStatus::Unhealthy,
    };

    match client.ping().await {
        Ok(()) => coil_observability::DependencyStatus::Healthy,
        Err(_) => coil_observability::DependencyStatus::Unhealthy,
    }
}

pub(super) fn maintenance_mode_json(
    mode: &coil_observability::MaintenanceMode,
) -> serde_json::Value {
    json!({
        "enabled": mode.enabled,
        "audience": match &mode.audience {
            coil_observability::MaintenanceAudience::Deployment => "deployment",
            coil_observability::MaintenanceAudience::CustomerApp(app_id) => app_id.as_str(),
        },
        "impact": match mode.impact {
            coil_observability::MaintenanceImpact::AllTraffic => "all_traffic",
            coil_observability::MaintenanceImpact::MutatingTrafficOnly => "mutating_traffic_only",
        },
        "allowed_background_work": mode
            .allowed_background_work
            .iter()
            .map(|work| format!("{work:?}"))
            .collect::<Vec<_>>(),
    })
}

fn health_status_string(status: coil_observability::DependencyStatus) -> &'static str {
    match status {
        coil_observability::DependencyStatus::Healthy => "healthy",
        coil_observability::DependencyStatus::Degraded => "degraded",
        coil_observability::DependencyStatus::Unhealthy => "unhealthy",
        coil_observability::DependencyStatus::Unknown => "unknown",
    }
}

fn live_queue_status(state: &RuntimeServerState) -> coil_observability::DependencyStatus {
    match live_jobs_snapshot(state) {
        Some(_) => coil_observability::DependencyStatus::Healthy,
        None => coil_observability::DependencyStatus::Unhealthy,
    }
}

fn live_jobs_snapshot(state: &RuntimeServerState) -> Option<JobsCoordinatorSnapshot> {
    state
        .plan
        .shared_jobs_runtime
        .get_or_init(&state.plan.jobs)
        .ok()
        .map(|runtime| runtime.snapshot())
}

fn live_distributed_cache_status(
    state: &RuntimeServerState,
) -> coil_observability::DependencyStatus {
    if live_cache_metrics(state).is_some() {
        coil_observability::DependencyStatus::Healthy
    } else {
        coil_observability::DependencyStatus::Unhealthy
    }
}

fn live_cache_metrics(state: &RuntimeServerState) -> Option<CacheMetrics> {
    let backend = state.backends.distributed_cache.as_ref()?;
    match backend.backend {
        coil_cache::DistributedCacheBackend::Redis => {
            std::env::var_os("REDIS_URL")?;
        }
        coil_cache::DistributedCacheBackend::Valkey => {
            if std::env::var_os("VALKEY_URL").is_none() && std::env::var_os("REDIS_URL").is_none()
            {
                return None;
            }
        }
    }
    let kind = match backend.backend {
        coil_cache::DistributedCacheBackend::Redis => CacheBackendKind::Redis,
        coil_cache::DistributedCacheBackend::Valkey => CacheBackendKind::Valkey,
    };
    catch_unwind(AssertUnwindSafe(|| {
        let runtime = coil_cache::DistributedCacheClient::live_shared_runtime(
            kind,
            state.plan.shared_backend_namespace(),
            PathBuf::new(),
        );
        runtime.metrics()
    }))
    .ok()
}

fn live_object_store_status(
    state: &RuntimeServerState,
) -> coil_observability::DependencyStatus {
    let Some(object_store) = state.backends.object_store.as_ref() else {
        return coil_observability::DependencyStatus::Unhealthy;
    };

    let Some(endpoint_url) = object_store.endpoint_url.as_deref() else {
        return coil_observability::DependencyStatus::Healthy;
    };

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(2))
        .timeout_read(std::time::Duration::from_secs(2))
        .build();
    match agent.request("HEAD", endpoint_url).call() {
        Ok(_) | Err(ureq::Error::Status(_, _)) => coil_observability::DependencyStatus::Healthy,
        Err(ureq::Error::Transport(_)) => coil_observability::DependencyStatus::Unhealthy,
    }
}

fn metric_reading_for_exposition(
    telemetry: &coil_observability::TelemetryCatalog,
    metric_name: &str,
    live_jobs_snapshot: Option<&JobsCoordinatorSnapshot>,
    live_cache_metrics: Option<&CacheMetrics>,
) -> Option<coil_observability::MetricReading> {
    match metric_name {
        "coil.queue.depth" => live_jobs_snapshot.map(|snapshot| {
            coil_observability::MetricReading::Gauge(
                (snapshot.ready.len() + snapshot.scheduled.len() + snapshot.in_flight.len())
                    as i64,
            )
        }),
        "coil.cache.hit_ratio" => live_cache_metrics.and_then(|metrics| {
            let total = metrics.hits + metrics.stale_hits + metrics.misses;
            (total > 0).then(|| {
                coil_observability::MetricReading::Gauge(
                    (((metrics.hits + metrics.stale_hits) * 100) / total) as i64,
                )
            })
        }),
        _ => telemetry.metric_reading(metric_name),
    }
}

fn prometheus_metrics_body(
    telemetry: &coil_observability::TelemetryCatalog,
    live_jobs_snapshot: Option<&JobsCoordinatorSnapshot>,
    live_cache_metrics: Option<&CacheMetrics>,
) -> String {
    let mut body = String::new();
    body.push_str(&format!(
        "# coil_metrics_enabled {}\n",
        if telemetry.metrics_enabled { 1 } else { 0 }
    ));
    body.push_str(&format!(
        "# coil_trace_enabled {}\n",
        if telemetry.trace.enabled { 1 } else { 0 }
    ));
    body.push_str(&format!(
        "# coil_trace_sample_permyriad {}\n",
        telemetry.trace.sample_permyriad
    ));

    if !telemetry.metrics_enabled {
        body.push_str("# metrics disabled by configuration\n");
        return body;
    }

    for metric in telemetry.metrics.values() {
        let metric_name = prometheus_metric_name(metric.name.as_str());
        match metric_reading_for_exposition(
            telemetry,
            metric.name.as_str(),
            live_jobs_snapshot,
            live_cache_metrics,
        ) {
            Some(coil_observability::MetricReading::Counter(value)) => {
                body.push_str(&format!("# TYPE {metric_name} counter\n"));
                body.push_str(&format!("{metric_name} {value}\n"));
            }
            Some(coil_observability::MetricReading::Gauge(value)) => {
                body.push_str(&format!("# TYPE {metric_name} gauge\n"));
                body.push_str(&format!("{metric_name} {value}\n"));
            }
            Some(coil_observability::MetricReading::Histogram(value)) => {
                body.push_str(&format!("# TYPE {metric_name}_samples counter\n"));
                body.push_str(&format!("{metric_name}_samples {}\n", value.samples));
                body.push_str(&format!("# TYPE {metric_name}_last gauge\n"));
                body.push_str(&format!("{metric_name}_last {}\n", value.last));
                body.push_str(&format!("# TYPE {metric_name}_max gauge\n"));
                body.push_str(&format!("{metric_name}_max {}\n", value.max));
            }
            None => {}
        }
    }

    if let Some(snapshot) = live_jobs_snapshot {
        body.push_str("# TYPE coil_runtime_jobs_ready gauge\n");
        body.push_str(&format!("coil_runtime_jobs_ready {}\n", snapshot.ready.len()));
        body.push_str("# TYPE coil_runtime_jobs_scheduled gauge\n");
        body.push_str(&format!(
            "coil_runtime_jobs_scheduled {}\n",
            snapshot.scheduled.len()
        ));
        body.push_str("# TYPE coil_runtime_jobs_in_flight gauge\n");
        body.push_str(&format!(
            "coil_runtime_jobs_in_flight {}\n",
            snapshot.in_flight.len()
        ));
        body.push_str("# TYPE coil_runtime_jobs_dead_letters gauge\n");
        body.push_str(&format!(
            "coil_runtime_jobs_dead_letters {}\n",
            snapshot.dead_letters.len()
        ));
    }

    if let Some(metrics) = live_cache_metrics {
        body.push_str("# TYPE coil_runtime_cache_hits counter\n");
        body.push_str(&format!("coil_runtime_cache_hits {}\n", metrics.hits));
        body.push_str("# TYPE coil_runtime_cache_stale_hits counter\n");
        body.push_str(&format!(
            "coil_runtime_cache_stale_hits {}\n",
            metrics.stale_hits
        ));
        body.push_str("# TYPE coil_runtime_cache_misses counter\n");
        body.push_str(&format!("coil_runtime_cache_misses {}\n", metrics.misses));
        body.push_str("# TYPE coil_runtime_cache_invalidations counter\n");
        body.push_str(&format!(
            "coil_runtime_cache_invalidations {}\n",
            metrics.invalidations
        ));
    }

    for trace in telemetry.recent_traces(25) {
        body.push_str(&format!(
            "# coil_trace trace_id={} span={} outcome={} recorded_at={}\n",
            trace.trace_id, trace.span, trace.outcome, trace.recorded_at_unix_seconds
        ));
    }

    body
}

fn prometheus_metric_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => ch,
            _ => '_',
        })
        .collect()
}

fn text_response(status: StatusCode, content_type: &'static str, body: String) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}
