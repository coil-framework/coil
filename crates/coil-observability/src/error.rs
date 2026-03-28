use crate::health::{DependencyKind, HealthProbeKind};
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
