mod backend;
mod coordinator;
mod domain;
mod error;
mod events;
mod identifiers;
mod model;
mod planner;
mod runtime;
mod validation;

#[cfg(test)]
mod tests;

pub use backend::{
    JobExecutionContext, JobFailureDisposition, JobLease, JobsBackendAdapter,
    JobsCoordinationRuntime, JobsCoordinatorSnapshot, QueuedJobRecord, SchedulerLeadership,
};
pub use coordinator::JobsCoordinator;
pub use domain::JobsDomain;
pub use error::JobsModelError;
pub use events::{DomainEventEnvelope, EventHandlerMetadata, EventSubscriptionMetadata};
pub use identifiers::{
    DeadLetterId, DomainEventId, DomainEventType, EventHandlerId, EventSubscriptionId,
    IdempotencyKey, JobId, JobName, JobQueueName, ScheduledJobId,
};
pub use model::{
    BackoffStrategy, DeadLetterOutcome, DeadLetterReason, JobInstant, QueueDefinition, QueueKind,
    QueueTopology, RetryPolicy,
};
pub use planner::JobsPlanner;
pub use runtime::{DeadLetterOutcomeKind, JobSpec, JobsRuntime, PlannedJob};
