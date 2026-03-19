use crate::validation::validate_token;
use crate::JobsModelError;
use std::fmt;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, JobsModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(JobId, "job_id");
token_type!(JobName, "job_name");
token_type!(JobQueueName, "queue_name");
token_type!(IdempotencyKey, "idempotency_key");
token_type!(ScheduledJobId, "scheduled_job_id");
token_type!(DeadLetterId, "dead_letter_id");
token_type!(DomainEventType, "domain_event_type");
token_type!(DomainEventId, "domain_event_id");
token_type!(EventHandlerId, "event_handler_id");
token_type!(EventSubscriptionId, "event_subscription_id");
