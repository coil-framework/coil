use super::*;
use std::collections::BTreeSet;

#[test]
fn telemetry_baseline_carries_required_metrics_dimensions_and_trace_policy() {
    let config = coil_config::ObservabilityConfig {
        metrics: true,
        tracing: true,
    };

    let runtime =
        ObservabilityRuntime::baseline(&config, coil_config::Environment::Production).unwrap();
    assert!(runtime.telemetry.metrics_enabled);
    assert!(runtime.telemetry.trace.enabled);
    assert_eq!(runtime.telemetry.trace.sample_permyriad, 1_000);
    assert!(
        runtime
            .telemetry
            .required_log_dimensions
            .contains(&DimensionKey::new("customer_app").unwrap())
    );
    assert!(
        runtime
            .telemetry
            .metric("coil.http.request.latency_ms")
            .is_some()
    );
    assert!(
        runtime
            .telemetry
            .metric("coil.http.requests.total")
            .is_some()
    );
    assert!(
        runtime
            .telemetry
            .metric("coil.http.requests.in_flight")
            .is_some()
    );
}

#[test]
fn readiness_degrades_for_unknown_or_failing_required_dependencies() {
    let readiness = HealthReport::new(HealthProbeKind::Readiness)
        .with_dependency(
            DependencyKind::DistributedCache,
            true,
            DependencyStatus::Healthy,
        )
        .unwrap()
        .with_dependency(DependencyKind::Queue, true, DependencyStatus::Unknown)
        .unwrap();
    assert_eq!(readiness.overall_status(), DependencyStatus::Degraded);

    let failing = readiness
        .with_dependency(DependencyKind::Queue, false, DependencyStatus::Healthy)
        .unwrap_err();
    assert_eq!(
        failing,
        ObservabilityError::DuplicateDependency {
            probe: HealthProbeKind::Readiness,
            dependency: DependencyKind::Queue,
        }
    );
}

#[test]
fn maintenance_mode_can_scope_to_mutations_and_customer_app() {
    let app = CustomerAppId::new("showcase").unwrap();
    let maintenance = MaintenanceMode {
        enabled: true,
        audience: MaintenanceAudience::CustomerApp(app.clone()),
        impact: MaintenanceImpact::MutatingTrafficOnly,
        bypass_token: Some("ops-bypass".to_string()),
        allowed_background_work: BTreeSet::from([BackgroundWorkClass::TlsRenewal]),
    };

    assert!(!maintenance.blocks_request(Some(&app), false, None));
    assert!(maintenance.blocks_request(Some(&app), true, None));
    assert!(!maintenance.blocks_request(Some(&app), true, Some("ops-bypass")));
    assert!(!maintenance.blocks_request(Some(&CustomerAppId::new("other").unwrap()), true, None));
}

#[test]
fn feature_flags_resolve_by_target_context() {
    let flag = FeatureFlag::new("new-checkout", false)
        .unwrap()
        .with_rule(
            FlagTarget::Environment(coil_config::Environment::Staging),
            true,
        )
        .unwrap()
        .with_rule(
            FlagTarget::CustomerApp(CustomerAppId::new("showcase").unwrap()),
            true,
        )
        .unwrap()
        .with_rule(FlagTarget::Cohort(CohortId::new("canary").unwrap()), false)
        .unwrap();

    let enabled_context = FeatureFlagContext {
        environment: coil_config::Environment::Production,
        customer_app: Some(CustomerAppId::new("showcase").unwrap()),
        site: None,
        brand: None,
        cohorts: BTreeSet::new(),
    };
    assert!(flag.enabled_for(&enabled_context));

    let canary_context = FeatureFlagContext {
        environment: coil_config::Environment::Production,
        customer_app: Some(CustomerAppId::new("showcase").unwrap()),
        site: None,
        brand: None,
        cohorts: BTreeSet::from([CohortId::new("canary").unwrap()]),
    };
    assert!(!flag.enabled_for(&canary_context));
}

#[test]
fn registry_rejects_duplicate_flags() {
    let mut registry = FeatureFlagRegistry::new();
    registry
        .insert(FeatureFlag::new("new-checkout", false).unwrap())
        .unwrap();

    let error = registry
        .insert(FeatureFlag::new("new-checkout", true).unwrap())
        .unwrap_err();
    assert_eq!(
        error,
        ObservabilityError::DuplicateFlag {
            flag: "new-checkout".to_string(),
        }
    );
}

#[test]
fn telemetry_catalog_records_live_counter_gauge_and_histogram_values() {
    let config = coil_config::ObservabilityConfig {
        metrics: true,
        tracing: true,
    };
    let runtime =
        ObservabilityRuntime::baseline(&config, coil_config::Environment::Development).unwrap();

    assert!(runtime.telemetry.increment_counter("coil.http.requests.total", 1));
    assert!(runtime.telemetry.adjust_gauge("coil.http.requests.in_flight", 1));
    assert!(runtime.telemetry.record_histogram("coil.http.request.latency_ms", 27));
    assert!(runtime.telemetry.set_gauge("coil.queue.depth", 5));
    assert!(runtime.telemetry.record_trace(
        TraceRecord::new("trace-1", "http.request", "ok", 42)
            .with_field("route", "home")
            .with_field("status", "200")
    ));

    assert_eq!(
        runtime.telemetry.metric_reading("coil.http.requests.total"),
        Some(MetricReading::Counter(1))
    );
    assert_eq!(
        runtime
            .telemetry
            .metric_reading("coil.http.requests.in_flight"),
        Some(MetricReading::Gauge(1))
    );
    assert_eq!(
        runtime
            .telemetry
            .metric_reading("coil.http.request.latency_ms"),
        Some(MetricReading::Histogram(HistogramReading {
            samples: 1,
            last: 27,
            max: 27,
        }))
    );
    assert_eq!(
        runtime.telemetry.metric_reading("coil.queue.depth"),
        Some(MetricReading::Gauge(5))
    );
    let traces = runtime.telemetry.recent_traces(10);
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].trace_id, "trace-1");
    assert_eq!(traces[0].span, "http.request");
    assert_eq!(traces[0].fields.get("route").map(String::as_str), Some("home"));
}
