use crate::error::OpsModelError;
use crate::identifiers::{ReportExportId, ReportId};
use crate::validation::require_non_empty;
use coil_auth::Capability;
use coil_jobs::{IdempotencyKey, JobInstant, JobSpec, PlannedJob};

use super::ReportDefinition;

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
