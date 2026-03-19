use crate::ObservabilityError;
use crate::health::ErrorCategory;
use crate::validation::{DimensionKey, MetricName};
use davenda_config::{Environment, ObservabilityConfig};
use std::collections::{BTreeMap, BTreeSet};

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
