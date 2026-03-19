use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use davenda_config::{Environment, ObservabilityConfig};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ObservabilityError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("metric `{metric}` is already defined")]
    DuplicateMetric { metric: String },
    #[error("probe `{probe}` already contains dependency `{dependency}`")]
    DuplicateDependency {
        probe: HealthProbeKind,
        dependency: DependencyKind,
    },
    #[error("feature flag `{flag}` is already defined")]
    DuplicateFlag { flag: String },
    #[error("feature flag `{flag}` contains duplicate rule `{scope}`")]
    DuplicateFlagRule { flag: String, scope: String },
    #[error("trace sample rate must be within 0..=10000, got `{permyriad}`")]
    InvalidTraceSampleRate { permyriad: u16 },
}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ObservabilityError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(MetricName, "metric_name");
token_type!(DimensionKey, "dimension_key");
token_type!(FeatureFlagId, "feature_flag_id");
token_type!(CustomerAppId, "customer_app_id");
token_type!(SiteId, "site_id");
token_type!(BrandId, "brand_id");
token_type!(CohortId, "cohort_id");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LogSeverity {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ErrorCategory {
    Validation,
    AuthorizationDenied,
    StateConflict,
    DependencyFailure,
    Timeout,
    Capacity,
    InvariantViolation,
    ExtensionTrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetricKind {
    Counter,
    Gauge,
    Histogram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetricUnit {
    Count,
    Milliseconds,
    Bytes,
    Ratio,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricDefinition {
    pub name: MetricName,
    pub kind: MetricKind,
    pub unit: MetricUnit,
    pub dimensions: BTreeSet<DimensionKey>,
}

impl MetricDefinition {
    pub fn new(
        name: impl Into<String>,
        kind: MetricKind,
        unit: MetricUnit,
    ) -> Result<Self, ObservabilityError> {
        Ok(Self {
            name: MetricName::new(name)?,
            kind,
            unit,
            dimensions: BTreeSet::new(),
        })
    }

    pub fn with_dimension(
        mut self,
        dimension: impl Into<String>,
    ) -> Result<Self, ObservabilityError> {
        self.dimensions.insert(DimensionKey::new(dimension)?);
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TracePolicy {
    pub enabled: bool,
    pub sample_permyriad: u16,
}

impl TracePolicy {
    pub fn new(enabled: bool, sample_permyriad: u16) -> Result<Self, ObservabilityError> {
        if sample_permyriad > 10_000 {
            return Err(ObservabilityError::InvalidTraceSampleRate {
                permyriad: sample_permyriad,
            });
        }

        Ok(Self {
            enabled,
            sample_permyriad,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryCatalog {
    pub metrics_enabled: bool,
    pub required_log_dimensions: BTreeSet<DimensionKey>,
    pub metrics: BTreeMap<MetricName, MetricDefinition>,
    pub trace: TracePolicy,
    pub error_categories: BTreeSet<ErrorCategory>,
}

impl TelemetryCatalog {
    pub fn baseline(
        config: &ObservabilityConfig,
        environment: Environment,
    ) -> Result<Self, ObservabilityError> {
        let mut metrics = BTreeMap::new();
        for metric in baseline_metrics()? {
            let name = metric.name.clone();
            if metrics.insert(name.clone(), metric).is_some() {
                return Err(ObservabilityError::DuplicateMetric {
                    metric: name.to_string(),
                });
            }
        }

        let mut dimensions = BTreeSet::new();
        for value in [
            "customer_app",
            "site",
            "brand",
            "route",
            "module",
            "extension_point",
            "outcome",
            "error_category",
        ] {
            dimensions.insert(DimensionKey::new(value)?);
        }

        let trace = TracePolicy::new(
            config.tracing,
            match environment {
                Environment::Development => 10_000,
                Environment::Staging => 5_000,
                Environment::Production => 1_000,
            },
        )?;

        Ok(Self {
            metrics_enabled: config.metrics,
            required_log_dimensions: dimensions,
            metrics,
            trace,
            error_categories: BTreeSet::from([
                ErrorCategory::Validation,
                ErrorCategory::AuthorizationDenied,
                ErrorCategory::StateConflict,
                ErrorCategory::DependencyFailure,
                ErrorCategory::Timeout,
                ErrorCategory::Capacity,
                ErrorCategory::InvariantViolation,
                ErrorCategory::ExtensionTrap,
            ]),
        })
    }

    pub fn metric(&self, name: &str) -> Option<&MetricDefinition> {
        self.metrics.get(&MetricName::new(name.to_string()).ok()?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HealthProbeKind {
    Liveness,
    Readiness,
    Synthetic,
}

impl fmt::Display for HealthProbeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Liveness => f.write_str("liveness"),
            Self::Readiness => f.write_str("readiness"),
            Self::Synthetic => f.write_str("synthetic"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyKind {
    Database,
    DistributedCache,
    Queue,
    ExtensionRegistry,
    ObjectStore,
    Secrets,
    Tls,
}

impl fmt::Display for DependencyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database => f.write_str("database"),
            Self::DistributedCache => f.write_str("distributed_cache"),
            Self::Queue => f.write_str("queue"),
            Self::ExtensionRegistry => f.write_str("extension_registry"),
            Self::ObjectStore => f.write_str("object_store"),
            Self::Secrets => f.write_str("secrets"),
            Self::Tls => f.write_str("tls"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeDependency {
    pub kind: DependencyKind,
    pub required: bool,
    pub status: DependencyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    pub kind: HealthProbeKind,
    pub dependencies: Vec<ProbeDependency>,
}

impl HealthReport {
    pub fn new(kind: HealthProbeKind) -> Self {
        Self {
            kind,
            dependencies: Vec::new(),
        }
    }

    pub fn with_dependency(
        mut self,
        kind: DependencyKind,
        required: bool,
        status: DependencyStatus,
    ) -> Result<Self, ObservabilityError> {
        if self
            .dependencies
            .iter()
            .any(|dependency| dependency.kind == kind)
        {
            return Err(ObservabilityError::DuplicateDependency {
                probe: self.kind,
                dependency: kind,
            });
        }

        self.dependencies.push(ProbeDependency {
            kind,
            required,
            status,
        });
        Ok(self)
    }

    pub fn overall_status(&self) -> DependencyStatus {
        if self.dependencies.iter().any(|dependency| {
            dependency.required && dependency.status == DependencyStatus::Unhealthy
        }) {
            return DependencyStatus::Unhealthy;
        }

        if self.dependencies.iter().any(|dependency| {
            dependency.required
                && matches!(
                    dependency.status,
                    DependencyStatus::Degraded | DependencyStatus::Unknown
                )
        }) {
            return DependencyStatus::Degraded;
        }

        DependencyStatus::Healthy
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BackgroundWorkClass {
    QueueDrain,
    TlsRenewal,
    StorageSync,
    WebhookRetry,
    SearchMaintenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaintenanceAudience {
    Deployment,
    CustomerApp(CustomerAppId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceImpact {
    AllTraffic,
    MutatingTrafficOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaintenanceMode {
    pub enabled: bool,
    pub audience: MaintenanceAudience,
    pub impact: MaintenanceImpact,
    pub bypass_token: Option<String>,
    pub allowed_background_work: BTreeSet<BackgroundWorkClass>,
}

impl MaintenanceMode {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            audience: MaintenanceAudience::Deployment,
            impact: MaintenanceImpact::AllTraffic,
            bypass_token: None,
            allowed_background_work: BTreeSet::new(),
        }
    }

    pub fn blocks_request(
        &self,
        customer_app: Option<&CustomerAppId>,
        method_is_mutating: bool,
        bypass_token: Option<&str>,
    ) -> bool {
        if !self.enabled {
            return false;
        }

        if self
            .bypass_token
            .as_deref()
            .is_some_and(|expected| Some(expected) == bypass_token)
        {
            return false;
        }

        let applies_to_app = match (&self.audience, customer_app) {
            (MaintenanceAudience::Deployment, _) => true,
            (MaintenanceAudience::CustomerApp(expected), Some(actual)) => expected == actual,
            (MaintenanceAudience::CustomerApp(_), None) => false,
        };

        if !applies_to_app {
            return false;
        }

        match self.impact {
            MaintenanceImpact::AllTraffic => true,
            MaintenanceImpact::MutatingTrafficOnly => method_is_mutating,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagTarget {
    Environment(Environment),
    CustomerApp(CustomerAppId),
    Site(SiteId),
    Brand(BrandId),
    Cohort(CohortId),
}

impl fmt::Display for FlagTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment(environment) => write!(f, "environment:{environment:?}"),
            Self::CustomerApp(app) => write!(f, "customer_app:{app}"),
            Self::Site(site) => write!(f, "site:{site}"),
            Self::Brand(brand) => write!(f, "brand:{brand}"),
            Self::Cohort(cohort) => write!(f, "cohort:{cohort}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlagRule {
    pub target: FlagTarget,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlag {
    pub id: FeatureFlagId,
    pub default_enabled: bool,
    pub rules: Vec<FeatureFlagRule>,
}

impl FeatureFlag {
    pub fn new(id: impl Into<String>, default_enabled: bool) -> Result<Self, ObservabilityError> {
        Ok(Self {
            id: FeatureFlagId::new(id)?,
            default_enabled,
            rules: Vec::new(),
        })
    }

    pub fn with_rule(
        mut self,
        target: FlagTarget,
        enabled: bool,
    ) -> Result<Self, ObservabilityError> {
        if self.rules.iter().any(|rule| rule.target == target) {
            return Err(ObservabilityError::DuplicateFlagRule {
                flag: self.id.to_string(),
                scope: target.to_string(),
            });
        }

        self.rules.push(FeatureFlagRule { target, enabled });
        Ok(self)
    }

    pub fn enabled_for(&self, context: &FeatureFlagContext) -> bool {
        self.rules
            .iter()
            .filter(|rule| context.matches(&rule.target))
            .map(|rule| rule.enabled)
            .next_back()
            .unwrap_or(self.default_enabled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlagContext {
    pub environment: Environment,
    pub customer_app: Option<CustomerAppId>,
    pub site: Option<SiteId>,
    pub brand: Option<BrandId>,
    pub cohorts: BTreeSet<CohortId>,
}

impl FeatureFlagContext {
    pub fn matches(&self, target: &FlagTarget) -> bool {
        match target {
            FlagTarget::Environment(environment) => &self.environment == environment,
            FlagTarget::CustomerApp(app) => self.customer_app.as_ref() == Some(app),
            FlagTarget::Site(site) => self.site.as_ref() == Some(site),
            FlagTarget::Brand(brand) => self.brand.as_ref() == Some(brand),
            FlagTarget::Cohort(cohort) => self.cohorts.contains(cohort),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FeatureFlagRegistry {
    flags: BTreeMap<FeatureFlagId, FeatureFlag>,
}

impl FeatureFlagRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, flag: FeatureFlag) -> Result<(), ObservabilityError> {
        if self.flags.insert(flag.id.clone(), flag.clone()).is_some() {
            return Err(ObservabilityError::DuplicateFlag {
                flag: flag.id.to_string(),
            });
        }

        Ok(())
    }

    pub fn get(&self, id: &FeatureFlagId) -> Option<&FeatureFlag> {
        self.flags.get(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservabilityRuntime {
    pub telemetry: TelemetryCatalog,
    pub liveness: HealthReport,
    pub readiness: HealthReport,
    pub maintenance: MaintenanceMode,
    pub flags: FeatureFlagRegistry,
}

impl ObservabilityRuntime {
    pub fn baseline(
        config: &ObservabilityConfig,
        environment: Environment,
    ) -> Result<Self, ObservabilityError> {
        Ok(Self {
            telemetry: TelemetryCatalog::baseline(config, environment)?,
            liveness: HealthReport::new(HealthProbeKind::Liveness),
            readiness: HealthReport::new(HealthProbeKind::Readiness),
            maintenance: MaintenanceMode::disabled(),
            flags: FeatureFlagRegistry::new(),
        })
    }
}

fn baseline_metrics() -> Result<Vec<MetricDefinition>, ObservabilityError> {
    let customer_dimensions = ["customer_app", "route", "outcome"];
    let storage_dimensions = ["customer_app", "module", "outcome"];
    let extension_dimensions = ["customer_app", "extension_point", "outcome"];

    Ok(vec![
        metric(
            "davenda.http.request.latency_ms",
            MetricKind::Histogram,
            MetricUnit::Milliseconds,
            &customer_dimensions,
        )?,
        metric(
            "davenda.auth.check.latency_ms",
            MetricKind::Histogram,
            MetricUnit::Milliseconds,
            &["customer_app", "module", "outcome"],
        )?,
        metric(
            "davenda.cache.hit_ratio",
            MetricKind::Gauge,
            MetricUnit::Ratio,
            &["customer_app", "module"],
        )?,
        metric(
            "davenda.queue.depth",
            MetricKind::Gauge,
            MetricUnit::Count,
            &["customer_app", "module"],
        )?,
        metric(
            "davenda.storage.sync.backlog",
            MetricKind::Gauge,
            MetricUnit::Count,
            &storage_dimensions,
        )?,
        metric(
            "davenda.tls.renewal.failures",
            MetricKind::Counter,
            MetricUnit::Count,
            &["customer_app", "outcome"],
        )?,
        metric(
            "davenda.extension.runtime_ms",
            MetricKind::Histogram,
            MetricUnit::Milliseconds,
            &extension_dimensions,
        )?,
    ])
}

fn metric(
    name: &str,
    kind: MetricKind,
    unit: MetricUnit,
    dimensions: &[&str],
) -> Result<MetricDefinition, ObservabilityError> {
    let mut definition = MetricDefinition::new(name, kind, unit)?;
    for dimension in dimensions {
        definition = definition.with_dimension(*dimension)?;
    }
    Ok(definition)
}

fn validate_token(field: &'static str, value: String) -> Result<String, ObservabilityError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ObservabilityError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(ObservabilityError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_baseline_carries_required_metrics_dimensions_and_trace_policy() {
        let config = ObservabilityConfig {
            metrics: true,
            tracing: true,
        };

        let runtime = ObservabilityRuntime::baseline(&config, Environment::Production).unwrap();
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
        assert!(!maintenance.blocks_request(
            Some(&CustomerAppId::new("other").unwrap()),
            true,
            None
        ));
    }

    #[test]
    fn feature_flags_resolve_by_target_context() {
        let flag = FeatureFlag::new("new-checkout", false)
            .unwrap()
            .with_rule(FlagTarget::Environment(Environment::Staging), true)
            .unwrap()
            .with_rule(
                FlagTarget::CustomerApp(CustomerAppId::new("showcase").unwrap()),
                true,
            )
            .unwrap()
            .with_rule(FlagTarget::Cohort(CohortId::new("canary").unwrap()), false)
            .unwrap();

        let enabled_context = FeatureFlagContext {
            environment: Environment::Production,
            customer_app: Some(CustomerAppId::new("showcase").unwrap()),
            site: None,
            brand: None,
            cohorts: BTreeSet::new(),
        };
        assert!(flag.enabled_for(&enabled_context));

        let canary_context = FeatureFlagContext {
            environment: Environment::Production,
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
}
