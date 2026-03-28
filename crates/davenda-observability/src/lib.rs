mod error;
mod feature_flags;
mod health;
mod runtime;
mod telemetry;
mod validation;

pub use error::ObservabilityError;
pub use feature_flags::{
    FeatureFlag, FeatureFlagContext, FeatureFlagRegistry, FeatureFlagRule, FlagTarget,
};
pub use health::{
    BackgroundWorkClass, DependencyKind, DependencyStatus, ErrorCategory, HealthProbeKind,
    HealthReport, LogSeverity, MaintenanceAudience, MaintenanceImpact, MaintenanceMode,
    ProbeDependency,
};
pub use runtime::ObservabilityRuntime;
pub use telemetry::{
    HistogramReading, MetricDefinition, MetricKind, MetricReading, MetricUnit, TelemetryCatalog,
    TracePolicy,
};
pub use validation::{
    BrandId, CohortId, CustomerAppId, DimensionKey, FeatureFlagId, MetricName, SiteId,
    validate_token,
};

#[cfg(test)]
mod tests;
