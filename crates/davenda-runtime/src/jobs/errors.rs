use super::super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeJobsError {
    #[error(transparent)]
    Jobs(#[from] JobsModelError),
    #[error(
        "live shared jobs backend `{backend:?}` requires an explicit distributed runtime; file-backed shared state is test-only"
    )]
    LiveSharedRuntimeRequiresExplicitBackend { backend: davenda_config::JobBackend },
    #[error("runtime value `{field}` cannot be empty")]
    EmptyValue { field: &'static str },
    #[error("job `{job}` is not declared by the runtime")]
    UnknownJob { job: String },
    #[error("job `{job}` must be dispatched through a domain event")]
    DomainEventJobRequiresEventDispatch { job: String },
    #[error("scheduled job `{job}` requires a scheduled execution instant")]
    ScheduledJobRequiresSchedule { job: String },
    #[error("job `{job}` uses trigger `{trigger:?}` and cannot be scheduled explicitly")]
    UnexpectedSchedule {
        job: String,
        trigger: JobTriggerKind,
    },
    #[error("job `{job}` requires an explicit idempotency key")]
    MissingIdempotencyKey { job: String },
}
