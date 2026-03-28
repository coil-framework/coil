use crate::ObservabilityError;
use crate::health::ErrorCategory;
use crate::validation::{DimensionKey, MetricName};
use coil_config::{Environment, ObservabilityConfig};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

const MAX_RECENT_TRACES: usize = 64;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistogramReading {
    pub samples: u64,
    pub last: u64,
    pub max: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricReading {
    Counter(u64),
    Gauge(i64),
    Histogram(HistogramReading),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceRecord {
    pub trace_id: String,
    pub span: String,
    pub outcome: String,
    pub recorded_at_unix_seconds: u64,
    pub fields: BTreeMap<String, String>,
}

impl TraceRecord {
    pub fn new(
        trace_id: impl Into<String>,
        span: impl Into<String>,
        outcome: impl Into<String>,
        recorded_at_unix_seconds: u64,
    ) -> Self {
        Self {
            trace_id: trace_id.into(),
            span: span.into(),
            outcome: outcome.into(),
            recorded_at_unix_seconds,
            fields: BTreeMap::new(),
        }
    }

    pub fn with_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Default)]
struct TelemetryState {
    readings: BTreeMap<MetricName, MetricReading>,
    recent_traces: VecDeque<TraceRecord>,
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

#[derive(Debug, Clone)]
pub struct TelemetryCatalog {
    pub metrics_enabled: bool,
    pub required_log_dimensions: BTreeSet<DimensionKey>,
    pub metrics: BTreeMap<MetricName, MetricDefinition>,
    pub trace: TracePolicy,
    pub error_categories: BTreeSet<ErrorCategory>,
    live_state: Arc<Mutex<TelemetryState>>,
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
        let initial_readings = initial_metric_readings(&metrics);

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
            live_state: Arc::new(Mutex::new(TelemetryState {
                readings: initial_readings,
                recent_traces: VecDeque::new(),
            })),
        })
    }

    pub fn metric(&self, name: &str) -> Option<&MetricDefinition> {
        self.metrics.get(&MetricName::new(name.to_string()).ok()?)
    }

    pub fn metric_reading(&self, name: &str) -> Option<MetricReading> {
        let metric = MetricName::new(name.to_string()).ok()?;
        self.live_state
            .lock()
            .expect("telemetry mutex poisoned")
            .readings
            .get(&metric)
            .copied()
    }

    pub fn increment_counter(&self, name: &str, delta: u64) -> bool {
        self.update_metric(name, |reading| match reading {
            MetricReading::Counter(value) => {
                *value = value.saturating_add(delta);
                true
            }
            _ => false,
        })
    }

    pub fn adjust_gauge(&self, name: &str, delta: i64) -> bool {
        self.update_metric(name, |reading| match reading {
            MetricReading::Gauge(value) => {
                *value = value.saturating_add(delta);
                true
            }
            _ => false,
        })
    }

    pub fn record_histogram(&self, name: &str, sample: u64) -> bool {
        self.update_metric(name, |reading| match reading {
            MetricReading::Histogram(value) => {
                value.samples = value.samples.saturating_add(1);
                value.last = sample;
                value.max = value.max.max(sample);
                true
            }
            _ => false,
        })
    }

    pub fn set_gauge(&self, name: &str, value: i64) -> bool {
        self.update_metric(name, |reading| match reading {
            MetricReading::Gauge(current) => {
                *current = value;
                true
            }
            _ => false,
        })
    }

    pub fn record_trace(&self, trace: TraceRecord) -> bool {
        if !self.trace.enabled {
            return false;
        }

        let mut state = self.live_state.lock().expect("telemetry mutex poisoned");
        if state.recent_traces.len() >= MAX_RECENT_TRACES {
            state.recent_traces.pop_front();
        }
        state.recent_traces.push_back(trace);
        true
    }

    pub fn recent_traces(&self, limit: usize) -> Vec<TraceRecord> {
        let state = self.live_state.lock().expect("telemetry mutex poisoned");
        state
            .recent_traces
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    fn update_metric(
        &self,
        name: &str,
        mut update: impl FnMut(&mut MetricReading) -> bool,
    ) -> bool {
        let Ok(metric_name) = MetricName::new(name.to_string()) else {
            return false;
        };
        let mut state = self.live_state.lock().expect("telemetry mutex poisoned");
        let Some(reading) = state.readings.get_mut(&metric_name) else {
            return false;
        };
        update(reading)
    }
}

impl PartialEq for TelemetryCatalog {
    fn eq(&self, other: &Self) -> bool {
        self.metrics_enabled == other.metrics_enabled
            && self.required_log_dimensions == other.required_log_dimensions
            && self.metrics == other.metrics
            && self.trace == other.trace
            && self.error_categories == other.error_categories
    }
}

impl Eq for TelemetryCatalog {}

fn baseline_metrics() -> Result<Vec<MetricDefinition>, ObservabilityError> {
    let customer_dimensions = ["customer_app", "route", "outcome"];
    let storage_dimensions = ["customer_app", "module", "outcome"];
    let extension_dimensions = ["customer_app", "extension_point", "outcome"];

    Ok(vec![
        metric(
            "coil.http.requests.total",
            MetricKind::Counter,
            MetricUnit::Count,
            &["customer_app", "outcome"],
        )?,
        metric(
            "coil.http.requests.in_flight",
            MetricKind::Gauge,
            MetricUnit::Count,
            &["customer_app"],
        )?,
        metric(
            "coil.http.request.latency_ms",
            MetricKind::Histogram,
            MetricUnit::Milliseconds,
            &customer_dimensions,
        )?,
        metric(
            "coil.auth.check.latency_ms",
            MetricKind::Histogram,
            MetricUnit::Milliseconds,
            &["customer_app", "module", "outcome"],
        )?,
        metric(
            "coil.cache.hit_ratio",
            MetricKind::Gauge,
            MetricUnit::Ratio,
            &["customer_app", "module"],
        )?,
        metric(
            "coil.queue.depth",
            MetricKind::Gauge,
            MetricUnit::Count,
            &["customer_app", "module"],
        )?,
        metric(
            "coil.storage.sync.backlog",
            MetricKind::Gauge,
            MetricUnit::Count,
            &storage_dimensions,
        )?,
        metric(
            "coil.tls.renewal.failures",
            MetricKind::Counter,
            MetricUnit::Count,
            &["customer_app", "outcome"],
        )?,
        metric(
            "coil.extension.runtime_ms",
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

fn initial_metric_readings(
    metrics: &BTreeMap<MetricName, MetricDefinition>,
) -> BTreeMap<MetricName, MetricReading> {
    metrics
        .iter()
        .map(|(name, definition)| {
            let reading = match definition.kind {
                MetricKind::Counter => MetricReading::Counter(0),
                MetricKind::Gauge => MetricReading::Gauge(0),
                MetricKind::Histogram => MetricReading::Histogram(HistogramReading {
                    samples: 0,
                    last: 0,
                    max: 0,
                }),
            };
            (name.clone(), reading)
        })
        .collect()
}
