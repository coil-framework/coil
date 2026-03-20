use davenda_auth::Capability;
use davenda_jobs::JobsModelError;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpsModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateIdentifier {
        kind: &'static str,
        id: String,
    },
    DuplicateField {
        index_id: String,
        field_id: String,
    },
    MissingCapability {
        operation: &'static str,
        required: Capability,
    },
    InvalidSearchVisibility {
        index_id: String,
        reason: String,
    },
    InvalidReportDelivery {
        report_id: String,
        reason: String,
    },
    InvalidBulkOperation {
        operation_id: String,
        reason: String,
    },
    InvalidRecoveryWorkflow {
        workflow_id: String,
        reason: String,
    },
    InvalidItemCount {
        operation: &'static str,
        count: usize,
    },
    MissingOperatorAcknowledgement {
        workflow_id: String,
        requirement: String,
    },
    JobsPlan {
        error: JobsModelError,
    },
}

impl fmt::Display for OpsModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateIdentifier { kind, id } => write!(f, "{kind} `{id}` is duplicated"),
            Self::DuplicateField { index_id, field_id } => {
                write!(f, "search index `{index_id}` duplicates field `{field_id}`")
            }
            Self::MissingCapability {
                operation,
                required,
            } => {
                write!(f, "{operation} requires capability `{required}`")
            }
            Self::InvalidSearchVisibility { index_id, reason } => {
                write!(
                    f,
                    "search index `{index_id}` has invalid visibility: {reason}"
                )
            }
            Self::InvalidReportDelivery { report_id, reason } => {
                write!(
                    f,
                    "report `{report_id}` has invalid delivery policy: {reason}"
                )
            }
            Self::InvalidBulkOperation {
                operation_id,
                reason,
            } => {
                write!(f, "bulk operation `{operation_id}` is invalid: {reason}")
            }
            Self::InvalidRecoveryWorkflow {
                workflow_id,
                reason,
            } => {
                write!(f, "recovery workflow `{workflow_id}` is invalid: {reason}")
            }
            Self::InvalidItemCount { operation, count } => {
                write!(f, "{operation} cannot target `{count}` items")
            }
            Self::MissingOperatorAcknowledgement {
                workflow_id,
                requirement,
            } => {
                write!(
                    f,
                    "recovery workflow `{workflow_id}` requires operator acknowledgement: {requirement}"
                )
            }
            Self::JobsPlan { error } => write!(f, "{error}"),
        }
    }
}

impl Error for OpsModelError {}

impl From<JobsModelError> for OpsModelError {
    fn from(error: JobsModelError) -> Self {
        Self::JobsPlan { error }
    }
}
