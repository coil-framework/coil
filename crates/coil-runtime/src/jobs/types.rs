use super::super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredModuleJob {
    pub module: String,
    pub job: JobContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredEventSubscription {
    pub module: String,
    pub subscription: EventSubscription,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSearchContribution {
    pub module: String,
    pub contribution: SearchIndexContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredDataRepository {
    pub module: String,
    pub contribution: DataRepositoryContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredReportDefinition {
    pub module: String,
    pub definition: ReportDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredBulkOperation {
    pub module: String,
    pub definition: BulkOperationDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeJobDefinition {
    pub module: String,
    pub contract: JobContract,
    pub queue: JobQueueName,
    pub retry_policy: RetryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventSubscriptionDefinition {
    pub module: String,
    pub event_type: DomainEventType,
    pub subscription_id: EventSubscriptionId,
    pub handler_id: EventHandlerId,
    pub job_name: String,
    pub reaction_queue: JobQueueName,
    pub retry_policy: RetryPolicy,
    pub target_trigger: JobTriggerKind,
    pub target_queue: JobQueueName,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventDispatch {
    pub event_id: DomainEventId,
    pub event_type: DomainEventType,
    pub enqueued_jobs: Vec<JobId>,
}
