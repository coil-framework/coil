use super::*;
use std::collections::BTreeSet;

#[test]
fn telemetry_baseline_carries_required_metrics_dimensions_and_trace_policy() {
    let config = davenda_config::ObservabilityConfig {
        metrics: true,
        tracing: true,
    };

    let runtime =
        ObservabilityRuntime::baseline(&config, davenda_config::Environment::Production).unwrap();
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
            .metric("davenda.http.request.latency_ms")
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
            FlagTarget::Environment(davenda_config::Environment::Staging),
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
        environment: davenda_config::Environment::Production,
        customer_app: Some(CustomerAppId::new("showcase").unwrap()),
        site: None,
        brand: None,
        cohorts: BTreeSet::new(),
    };
    assert!(flag.enabled_for(&enabled_context));

    let canary_context = FeatureFlagContext {
        environment: davenda_config::Environment::Production,
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
