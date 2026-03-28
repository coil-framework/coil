use crate::ObservabilityError;
use crate::feature_flags::FeatureFlagRegistry;
use crate::health::{HealthProbeKind, HealthReport, MaintenanceMode};
use crate::telemetry::TelemetryCatalog;
use coil_config::{Environment, ObservabilityConfig};

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
