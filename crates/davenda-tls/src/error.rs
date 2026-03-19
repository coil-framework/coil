use thiserror::Error;

use crate::TlsInstant;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TlsModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("external termination does not issue certificates")]
    ExternalTerminationDoesNotIssue,
    #[error("manual mode requires an imported certificate inventory entry")]
    ManualModeRequiresImportedCertificate,
    #[error("wildcard hostnames require dns-01 validation")]
    WildcardRequiresDns01,
    #[error("certificate `{certificate_id}` is not currently active")]
    CertificateNotActive { certificate_id: String },
    #[error(
        "certificate `{certificate_id}` cannot be renewed because it is already replacing itself"
    )]
    RenewalAlreadyInProgress { certificate_id: String },
    #[error("certificate `{certificate_id}` is not known to the TLS inventory")]
    UnknownCertificate { certificate_id: String },
    #[error("hostname `{hostname}` is already bound to active certificate `{certificate_id}`")]
    DuplicateHostnameBinding {
        hostname: String,
        certificate_id: String,
    },
    #[error(
        "certificate `{certificate_id}` cannot be renewed until `{renew_after}`, current time is `{now}`"
    )]
    RenewalNotDue {
        certificate_id: String,
        renew_after: TlsInstant,
        now: TlsInstant,
    },
    #[error("certificate `{certificate_id}` has no pending replacement")]
    MissingReplacementCertificate { certificate_id: String },
    #[error("tls automation state `{path}` is invalid: {reason}")]
    CorruptAutomationState { path: String, reason: String },
    #[error("failed to persist tls automation state `{path}`: {reason}")]
    AutomationStatePersistence { path: String, reason: String },
    #[error("tls automation state namespace `{namespace}` is invalid: {reason}")]
    CorruptSharedAutomationState { namespace: String, reason: String },
    #[error("failed to persist shared tls automation state `{namespace}`: {reason}")]
    SharedAutomationStatePersistence { namespace: String, reason: String },
}
