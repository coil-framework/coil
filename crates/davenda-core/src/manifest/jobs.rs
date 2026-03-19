#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobTriggerKind {
    Scheduled,
    DomainEvent,
    Operator,
    Webhook,
    InlineFollowup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobContract {
    pub name: String,
    pub trigger: JobTriggerKind,
    pub idempotent: bool,
    pub description: String,
}

impl JobContract {
    pub fn new(
        name: impl Into<String>,
        trigger: JobTriggerKind,
        idempotent: bool,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            trigger,
            idempotent,
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSubscription {
    pub event: String,
    pub job: Option<String>,
    pub description: String,
}

impl EventSubscription {
    pub fn new(
        event: impl Into<String>,
        job: Option<impl Into<String>>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            event: event.into(),
            job: job.map(Into::into),
            description: description.into(),
        }
    }
}
