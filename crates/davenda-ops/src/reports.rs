use crate::error::OpsModelError;
use crate::identifiers::{ReportExportId, ReportId};
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_core::{
    ModuleManifest, ReportDefinition as ManifestReportDefinition,
    ReportDeliveryMode as ManifestReportDeliveryMode, ReportFormat as ManifestReportFormat,
    ReportSensitivity as ManifestReportSensitivity,
};
use davenda_jobs::{IdempotencyKey, JobInstant, JobSpec, PlannedJob, RetryPolicy};
use std::collections::HashSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Csv,
    Json,
    Pdf,
}

impl ReportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Pdf => "pdf",
        }
    }
}

impl fmt::Display for ReportFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv => f.write_str("csv"),
            Self::Json => f.write_str("json"),
            Self::Pdf => f.write_str("pdf"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSensitivity {
    Public,
    Internal,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportDeliveryMode {
    PublicObjectStore,
    SignedUrl,
    InternalOnly,
}

impl ReportDeliveryMode {
    pub fn is_public(self) -> bool {
        matches!(self, Self::PublicObjectStore)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportParameter {
    pub key: String,
    pub value: String,
}

impl ReportParameter {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, OpsModelError> {
        Ok(Self {
            key: require_non_empty("report_parameter_key", key.into())?,
            value: require_non_empty("report_parameter_value", value.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDefinition {
    pub id: ReportId,
    pub source_module: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub format: ReportFormat,
    pub sensitivity: ReportSensitivity,
    pub delivery_mode: ReportDeliveryMode,
    pub export_prefix: String,
    pub retry_policy: RetryPolicy,
}

impl ReportDefinition {
    pub fn new(
        id: ReportId,
        source_module: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        format: ReportFormat,
        sensitivity: ReportSensitivity,
        delivery_mode: ReportDeliveryMode,
        export_prefix: impl Into<String>,
        retry_policy: RetryPolicy,
    ) -> Result<Self, OpsModelError> {
        let source_module = require_non_empty("report_source_module", source_module.into())?;
        let title = require_non_empty("report_title", title.into())?;
        let export_prefix = require_non_empty("report_export_prefix", export_prefix.into())?;

        if matches!(
            (sensitivity, delivery_mode),
            (ReportSensitivity::Public, ReportDeliveryMode::InternalOnly)
        ) {
            return Err(OpsModelError::InvalidReportDelivery {
                report_id: id.to_string(),
                reason:
                    "public reports should be delivered through a public object store or signed URL"
                        .to_string(),
            });
        }

        if matches!(sensitivity, ReportSensitivity::Restricted)
            && matches!(delivery_mode, ReportDeliveryMode::PublicObjectStore)
        {
            return Err(OpsModelError::InvalidReportDelivery {
                report_id: id.to_string(),
                reason: "restricted reports cannot be delivered through a public object store"
                    .to_string(),
            });
        }

        Ok(Self {
            id,
            source_module,
            title,
            description,
            required_capability,
            format,
            sensitivity,
            delivery_mode,
            export_prefix,
            retry_policy,
        })
    }

    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        capabilities.contains(&self.required_capability)
    }

    pub(crate) fn from_manifest_definition(
        source_module: &str,
        definition: &ManifestReportDefinition,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            ReportId::new(definition.id.clone())?,
            source_module,
            definition.title.clone(),
            definition.description.clone(),
            definition.required_capability,
            map_report_format(definition.format),
            map_report_sensitivity(definition.sensitivity),
            map_report_delivery_mode(definition.delivery_mode),
            definition.export_prefix.clone(),
            definition.retry_policy.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportExportRequest {
    pub export_id: ReportExportId,
    pub report_id: ReportId,
    pub requested_by: String,
    pub requested_at: JobInstant,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<IdempotencyKey>,
    pub operator_capabilities: Vec<Capability>,
    pub parameters: Vec<ReportParameter>,
}

impl ReportExportRequest {
    pub fn new(
        export_id: ReportExportId,
        report_id: ReportId,
        requested_by: impl Into<String>,
        requested_at: JobInstant,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            export_id,
            report_id,
            requested_by: require_non_empty("report_requested_by", requested_by.into())?,
            requested_at,
            scheduled_for: None,
            idempotency_key: None,
            operator_capabilities: Vec::new(),
            parameters: Vec::new(),
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_idempotency_key(mut self, key: IdempotencyKey) -> Self {
        self.idempotency_key = Some(key);
        self
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.operator_capabilities.push(capability);
        self
    }

    pub fn with_parameter(mut self, parameter: ReportParameter) -> Self {
        self.parameters.push(parameter);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportExportPlan {
    pub definition: ReportDefinition,
    pub job: JobSpec,
    pub planned_job: PlannedJob,
    pub output_object_key: String,
    pub parameters: Vec<ReportParameter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportCatalog {
    pub definitions: Vec<ReportDefinition>,
}

impl ReportCatalog {
    pub fn new(definitions: Vec<ReportDefinition>) -> Self {
        Self { definitions }
    }

    pub fn standard() -> Self {
        Self {
            definitions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut definitions = Vec::new();
        for manifest in manifests {
            for definition in &manifest.report_definitions {
                definitions.push(ReportDefinition::from_manifest_definition(
                    &manifest.name,
                    definition,
                )?);
            }
        }
        let catalog = Self::new(definitions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for definition in &self.definitions {
            if !seen.insert(definition.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "report",
                    id: definition.id.to_string(),
                });
            }

            ReportDefinition::new(
                definition.id.clone(),
                definition.source_module.clone(),
                definition.title.clone(),
                definition.description.clone(),
                definition.required_capability,
                definition.format,
                definition.sensitivity,
                definition.delivery_mode,
                definition.export_prefix.clone(),
                definition.retry_policy.clone(),
            )?;
        }

        Ok(())
    }

    pub fn definition(&self, id: &ReportId) -> Option<&ReportDefinition> {
        self.definitions
            .iter()
            .find(|definition| &definition.id == id)
    }
}

impl Default for ReportCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

fn map_report_format(format: ManifestReportFormat) -> ReportFormat {
    match format {
        ManifestReportFormat::Csv => ReportFormat::Csv,
        ManifestReportFormat::Json => ReportFormat::Json,
        ManifestReportFormat::Pdf => ReportFormat::Pdf,
    }
}

fn map_report_sensitivity(sensitivity: ManifestReportSensitivity) -> ReportSensitivity {
    match sensitivity {
        ManifestReportSensitivity::Public => ReportSensitivity::Public,
        ManifestReportSensitivity::Internal => ReportSensitivity::Internal,
        ManifestReportSensitivity::Restricted => ReportSensitivity::Restricted,
    }
}

fn map_report_delivery_mode(mode: ManifestReportDeliveryMode) -> ReportDeliveryMode {
    match mode {
        ManifestReportDeliveryMode::PublicObjectStore => ReportDeliveryMode::PublicObjectStore,
        ManifestReportDeliveryMode::SignedUrl => ReportDeliveryMode::SignedUrl,
        ManifestReportDeliveryMode::InternalOnly => ReportDeliveryMode::InternalOnly,
    }
}
