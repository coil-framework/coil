use crate::ObservabilityError;
use crate::validation::CustomerAppId;
use std::collections::BTreeSet;
use std::fmt;

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

    pub fn dependency(&self, kind: DependencyKind) -> Option<ProbeDependency> {
        self.dependencies
            .iter()
            .find(|dependency| dependency.kind == kind)
            .copied()
    }

    pub fn set_dependency_status(&mut self, kind: DependencyKind, status: DependencyStatus) -> bool {
        let Some(dependency) = self
            .dependencies
            .iter_mut()
            .find(|dependency| dependency.kind == kind)
        else {
            return false;
        };
        dependency.status = status;
        true
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
