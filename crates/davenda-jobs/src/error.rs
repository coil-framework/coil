use crate::model::{JobInstant, QueueKind};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobsModelError {
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
    UnknownQueue {
        queue: String,
    },
    QueueKindMismatch {
        queue: String,
        expected: QueueKind,
        actual: QueueKind,
    },
    MissingIdempotencyKey {
        job_id: String,
    },
    InvalidRetryPolicy {
        reason: String,
    },
    ScheduledInPast {
        scheduled_at: JobInstant,
        now: JobInstant,
    },
    MissingDeadLetterQueue {
        queue: String,
    },
    LeadershipConflict {
        current_holder: String,
        requested_holder: String,
    },
    MissingSchedulerLeadership {
        node_id: String,
    },
    SchedulerLeadershipExpired {
        node_id: String,
        lease_until: JobInstant,
        now: JobInstant,
    },
    UnknownInFlightJob {
        job_id: String,
    },
    LeaseExpired {
        job_id: String,
        lease_until: JobInstant,
        now: JobInstant,
    },
    MissingEventHandler {
        handler_id: String,
    },
}

impl fmt::Display for JobsModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateIdentifier { kind, id } => write!(f, "{kind} `{id}` is duplicated"),
            Self::UnknownQueue { queue } => write!(f, "queue `{queue}` is not registered"),
            Self::QueueKindMismatch {
                queue,
                expected,
                actual,
            } => write!(
                f,
                "queue `{queue}` has kind `{actual}` but `{expected}` was required"
            ),
            Self::MissingIdempotencyKey { job_id } => {
                write!(f, "job `{job_id}` requires an idempotency key for retries")
            }
            Self::InvalidRetryPolicy { reason } => write!(f, "invalid retry policy: {reason}"),
            Self::ScheduledInPast { scheduled_at, now } => {
                write!(
                    f,
                    "scheduled time `{scheduled_at}` must be later than `{now}`"
                )
            }
            Self::MissingDeadLetterQueue { queue } => {
                write!(f, "queue `{queue}` requires a dead-letter queue")
            }
            Self::LeadershipConflict {
                current_holder,
                requested_holder,
            } => write!(
                f,
                "scheduler leadership is held by `{current_holder}`, `{requested_holder}` cannot take it"
            ),
            Self::MissingSchedulerLeadership { node_id } => {
                write!(f, "node `{node_id}` does not hold scheduler leadership")
            }
            Self::SchedulerLeadershipExpired {
                node_id,
                lease_until,
                now,
            } => write!(
                f,
                "scheduler leadership for `{node_id}` expired at `{lease_until}`, current time is `{now}`"
            ),
            Self::UnknownInFlightJob { job_id } => {
                write!(f, "job `{job_id}` is not currently leased")
            }
            Self::LeaseExpired {
                job_id,
                lease_until,
                now,
            } => write!(
                f,
                "lease for job `{job_id}` expired at `{lease_until}`, current time is `{now}`"
            ),
            Self::MissingEventHandler { handler_id } => {
                write!(f, "event handler `{handler_id}` is not registered")
            }
        }
    }
}

impl Error for JobsModelError {}
