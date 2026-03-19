use super::super::*;
use super::helpers::validate_runtime_identifier;
use super::RuntimeJobsError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobDispatchRequest {
    pub job_name: String,
    pub payload_description: String,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<String>,
}

impl JobDispatchRequest {
    pub fn new(
        job_name: impl Into<String>,
        payload_description: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        let job_name = validate_runtime_identifier("job_name", job_name.into())?;
        let payload_description =
            validate_runtime_identifier("payload_description", payload_description.into())?;

        Ok(Self {
            job_name,
            payload_description,
            scheduled_for: None,
            idempotency_key: None,
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_idempotency_key(
        mut self,
        key: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        self.idempotency_key = Some(validate_runtime_identifier("idempotency_key", key.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventDispatchRequest {
    pub event_type: String,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub payload_description: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

impl DomainEventDispatchRequest {
    pub fn new(
        event_type: impl Into<String>,
        aggregate_kind: impl Into<String>,
        aggregate_id: impl Into<String>,
        payload_description: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        Ok(Self {
            event_type: validate_runtime_identifier("event_type", event_type.into())?,
            aggregate_kind: validate_runtime_identifier("aggregate_kind", aggregate_kind.into())?,
            aggregate_id: validate_runtime_identifier("aggregate_id", aggregate_id.into())?,
            payload_description: validate_runtime_identifier(
                "payload_description",
                payload_description.into(),
            )?,
            correlation_id: None,
            causation_id: None,
        })
    }

    pub fn with_correlation_id(
        mut self,
        correlation_id: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        self.correlation_id = Some(validate_runtime_identifier(
            "correlation_id",
            correlation_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_causation_id(
        mut self,
        causation_id: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        self.causation_id = Some(validate_runtime_identifier(
            "causation_id",
            causation_id.into(),
        )?);
        Ok(self)
    }
}
