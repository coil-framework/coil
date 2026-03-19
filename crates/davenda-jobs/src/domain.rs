use crate::error::JobsModelError;
use crate::events::{EventHandlerMetadata, EventSubscriptionMetadata};
use crate::runtime::JobsRuntime;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobsDomain {
    pub runtime: JobsRuntime,
    pub domain_event_subscriptions: Vec<EventSubscriptionMetadata>,
    pub handlers: Vec<EventHandlerMetadata>,
}

impl JobsDomain {
    pub fn new(runtime: JobsRuntime) -> Self {
        Self {
            runtime,
            domain_event_subscriptions: Vec::new(),
            handlers: Vec::new(),
        }
    }

    pub fn add_subscription(mut self, subscription: EventSubscriptionMetadata) -> Self {
        self.domain_event_subscriptions.push(subscription);
        self
    }

    pub fn add_handler(mut self, handler: EventHandlerMetadata) -> Self {
        self.handlers.push(handler);
        self
    }

    pub fn validate(&self) -> Result<(), JobsModelError> {
        self.runtime.topology.validate()?;

        let mut seen = BTreeSet::new();
        for subscription in &self.domain_event_subscriptions {
            if !seen.insert(subscription.id.as_str()) {
                return Err(JobsModelError::DuplicateIdentifier {
                    kind: "event subscription",
                    id: subscription.id.to_string(),
                });
            }
        }

        for subscription in &self.domain_event_subscriptions {
            self.runtime
                .topology
                .queue(&subscription.queue)
                .ok_or_else(|| JobsModelError::UnknownQueue {
                    queue: subscription.queue.to_string(),
                })?;

            if subscription.retry_policy.is_retrying() && subscription.idempotency_key.is_none() {
                return Err(JobsModelError::MissingIdempotencyKey {
                    job_id: subscription.id.to_string(),
                });
            }
        }

        for handler in &self.handlers {
            self.runtime.topology.queue(&handler.queue).ok_or_else(|| {
                JobsModelError::UnknownQueue {
                    queue: handler.queue.to_string(),
                }
            })?;
        }

        Ok(())
    }
}
