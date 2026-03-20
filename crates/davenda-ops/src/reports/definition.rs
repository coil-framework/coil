use crate::error::OpsModelError;
use crate::identifiers::ReportId;
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_core::{
    ReportDefinition as ManifestReportDefinition, ReportDeliveryMode as ManifestReportDeliveryMode,
    ReportFormat as ManifestReportFormat, ReportSensitivity as ManifestReportSensitivity,
};
use davenda_jobs::RetryPolicy;
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

pub(crate) fn map_report_format(format: ManifestReportFormat) -> ReportFormat {
    match format {
        ManifestReportFormat::Csv => ReportFormat::Csv,
        ManifestReportFormat::Json => ReportFormat::Json,
        ManifestReportFormat::Pdf => ReportFormat::Pdf,
    }
}

pub(crate) fn map_report_sensitivity(sensitivity: ManifestReportSensitivity) -> ReportSensitivity {
    match sensitivity {
        ManifestReportSensitivity::Public => ReportSensitivity::Public,
        ManifestReportSensitivity::Internal => ReportSensitivity::Internal,
        ManifestReportSensitivity::Restricted => ReportSensitivity::Restricted,
    }
}

pub(crate) fn map_report_delivery_mode(mode: ManifestReportDeliveryMode) -> ReportDeliveryMode {
    match mode {
        ManifestReportDeliveryMode::PublicObjectStore => ReportDeliveryMode::PublicObjectStore,
        ManifestReportDeliveryMode::SignedUrl => ReportDeliveryMode::SignedUrl,
        ManifestReportDeliveryMode::InternalOnly => ReportDeliveryMode::InternalOnly,
    }
}
