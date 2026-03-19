use super::*;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventDispatch {
    pub event_id: DomainEventId,
    pub event_type: DomainEventType,
    pub enqueued_jobs: Vec<JobId>,
}

#[derive(Debug, Clone)]
pub struct JobsHost {
    pub customer_app: String,
    pub scheduler_node_id: String,
    pub runtime: JobsRuntimeServices,
    pub queue_topology: QueueTopology,
    pub registered_jobs: Vec<RuntimeJobDefinition>,
    pub registered_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
    pub jobs_domain: JobsDomain,
    coordinator: JobsCoordinator,
    next_job_sequence: u64,
    next_event_sequence: u64,
}

impl JobsHost {
    pub(crate) fn new(
        customer_app: String,
        scheduler_node_id: String,
        runtime: JobsRuntimeServices,
        queue_topology: QueueTopology,
        registered_jobs: Vec<RuntimeJobDefinition>,
        registered_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
        jobs_domain: JobsDomain,
    ) -> Self {
        #[cfg(test)]
        let coordinator = runtime.coordinator();
        #[cfg(not(test))]
        let coordinator =
            runtime.coordinator_with_backend(davenda_jobs::JobsBackendAdapter::shared(&runtime));
        Self {
            customer_app,
            scheduler_node_id,
            runtime,
            queue_topology,
            registered_jobs,
            registered_event_subscriptions,
            jobs_domain,
            coordinator,
            next_job_sequence: 0,
            next_event_sequence: 0,
        }
    }

    pub fn enqueue_spec(
        &mut self,
        spec: JobSpec,
        now: JobInstant,
    ) -> Result<JobId, RuntimeJobsError> {
        let job_id = spec.job_id.clone();
        self.coordinator.enqueue(spec, now)?;
        Ok(job_id)
    }

    pub fn enqueue_job(
        &mut self,
        request: JobDispatchRequest,
        now: JobInstant,
    ) -> Result<JobId, RuntimeJobsError> {
        let Some(definition) = self
            .registered_jobs
            .iter()
            .find(|definition| definition.contract.name == request.job_name)
            .cloned()
        else {
            return Err(RuntimeJobsError::UnknownJob {
                job: request.job_name,
            });
        };

        match definition.contract.trigger {
            JobTriggerKind::Scheduled if request.scheduled_for.is_none() => {
                return Err(RuntimeJobsError::ScheduledJobRequiresSchedule {
                    job: definition.contract.name,
                });
            }
            JobTriggerKind::Scheduled => {}
            JobTriggerKind::DomainEvent => {
                return Err(RuntimeJobsError::DomainEventJobRequiresEventDispatch {
                    job: definition.contract.name,
                });
            }
            trigger if request.scheduled_for.is_some() => {
                return Err(RuntimeJobsError::UnexpectedSchedule {
                    job: definition.contract.name,
                    trigger,
                });
            }
            _ => {}
        }

        let mut spec = JobSpec::new(
            self.issue_job_id(&definition.contract.name)?,
            JobName::new(definition.contract.name.clone())?,
            definition.queue.clone(),
            request.payload_description,
        )?
        .with_retry_policy(definition.retry_policy.clone());

        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }

        match request.idempotency_key {
            Some(key) => {
                spec = spec.with_idempotency_key(IdempotencyKey::new(key)?);
            }
            None if definition.retry_policy.is_retrying() => {
                return Err(RuntimeJobsError::MissingIdempotencyKey {
                    job: definition.contract.name,
                });
            }
            None => {}
        }

        let job_id = spec.job_id.clone();
        self.coordinator.enqueue(spec, now)?;
        Ok(job_id)
    }

    pub fn emit_domain_event(
        &mut self,
        request: DomainEventDispatchRequest,
        now: JobInstant,
    ) -> Result<DomainEventDispatch, RuntimeJobsError> {
        let event_type = DomainEventType::new(request.event_type.clone())?;
        let event_id = self.issue_event_id(&request.event_type)?;
        let mut envelope = DomainEventEnvelope::new(
            event_id.clone(),
            event_type.clone(),
            request.aggregate_kind,
            request.aggregate_id,
            now,
            request.payload_description,
        )?;

        if let Some(correlation_id) = request.correlation_id {
            envelope = envelope.with_correlation_id(correlation_id)?;
        }

        if let Some(causation_id) = request.causation_id {
            envelope = envelope.with_causation_id(causation_id)?;
        }

        let mut enqueued_jobs = Vec::new();
        for subscription in self
            .registered_event_subscriptions
            .iter()
            .filter(|subscription| subscription.event_type == event_type)
            .cloned()
        {
            let mut spec = JobSpec::new(
                JobId::new(format!(
                    "event:{}:{}",
                    event_id.as_str(),
                    subscription.subscription_id.as_str()
                ))?,
                JobName::new(format!("event-handler:{}", subscription.job_name))?,
                subscription.reaction_queue,
                format!(
                    "dispatch {} for {}:{}",
                    event_type.as_str(),
                    envelope.aggregate_kind,
                    envelope.aggregate_id
                ),
            )?
            .with_retry_policy(subscription.retry_policy.clone());

            if subscription.retry_policy.is_retrying() {
                spec = spec.with_idempotency_key(IdempotencyKey::new(format!(
                    "event:{}:{}:{}",
                    event_id.as_str(),
                    subscription.module,
                    subscription.job_name
                ))?);
            }

            let job_id = spec.job_id.clone();
            self.coordinator.enqueue(spec, now)?;
            enqueued_jobs.push(job_id);
        }

        Ok(DomainEventDispatch {
            event_id,
            event_type,
            enqueued_jobs,
        })
    }

    pub fn acquire_scheduler_leadership(
        &mut self,
        now: JobInstant,
        lease_ttl: std::time::Duration,
    ) -> Result<SchedulerLeadership, RuntimeJobsError> {
        Ok(self.coordinator.acquire_scheduler_leadership(
            self.scheduler_node_id.clone(),
            now,
            lease_ttl,
        )?)
    }

    pub fn promote_due_jobs(&mut self, now: JobInstant) -> Result<Vec<JobId>, RuntimeJobsError> {
        Ok(self
            .coordinator
            .promote_due_jobs(&self.scheduler_node_id, now)?)
    }

    pub fn lease_ready_jobs(
        &mut self,
        queue: &JobQueueName,
        worker_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: std::time::Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, RuntimeJobsError> {
        Ok(self
            .coordinator
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)?)
    }

    pub fn acknowledge_completed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), RuntimeJobsError> {
        Ok(self.coordinator.acknowledge_completed(lease, now)?)
    }

    pub fn acknowledge_failed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: impl Into<String>,
    ) -> Result<JobFailureDisposition, RuntimeJobsError> {
        Ok(self
            .coordinator
            .acknowledge_failed(lease, now, reason, error_message.into())?)
    }

    pub fn coordinator(&self) -> &JobsCoordinator {
        &self.coordinator
    }

    fn issue_job_id(&mut self, job_name: &str) -> Result<JobId, RuntimeJobsError> {
        self.next_job_sequence += 1;
        Ok(JobId::new(format!(
            "job:{}:{}",
            job_name, self.next_job_sequence
        ))?)
    }

    fn issue_event_id(&mut self, event_type: &str) -> Result<DomainEventId, RuntimeJobsError> {
        self.next_event_sequence += 1;
        Ok(DomainEventId::new(format!(
            "evt:{}:{}",
            event_type, self.next_event_sequence
        ))?)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeJobsError {
    #[error(transparent)]
    Jobs(#[from] JobsModelError),
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

pub(crate) fn collect_extension_runtime_jobs(
    extension_registry: &ExtensionRegistry,
) -> Result<Vec<RegisteredModuleJob>, RuntimeBuildError> {
    let mut extensions_by_id = BTreeMap::new();
    for extension in extension_registry.extensions() {
        extensions_by_id.insert(extension.manifest().id.to_string(), extension);
    }

    let mut jobs = Vec::new();
    for handler in extension_registry.registered_handlers() {
        if !matches!(
            handler.point,
            ExtensionPointKind::Job | ExtensionPointKind::ScheduledJob
        ) {
            continue;
        }

        let extension = extensions_by_id
            .get(&handler.extension_id.to_string())
            .expect("registered handlers always belong to an installed extension");
        let manifest_handler = extension
            .manifest()
            .handler(&handler.handler_id)
            .expect("registered handlers always belong to a manifest handler");

        let contract = match &manifest_handler.point {
            davenda_wasm::ExtensionPoint::Job(job) => JobContract::new(
                job.job_name.clone(),
                JobTriggerKind::Operator,
                false,
                format!(
                    "WASM extension job `{}` from `{}`",
                    handler.handler_id,
                    extension.manifest().id
                ),
            ),
            davenda_wasm::ExtensionPoint::ScheduledJob(job) => JobContract::new(
                job.job_name.clone(),
                JobTriggerKind::Scheduled,
                false,
                format!(
                    "WASM scheduled job `{}` from `{}` on `{}`",
                    handler.handler_id,
                    extension.manifest().id,
                    job.schedule
                ),
            ),
            _ => continue,
        };

        jobs.push(RegisteredModuleJob {
            module: format!("extension:{}", extension.manifest().id),
            job: contract,
        });
    }

    Ok(jobs)
}

pub(crate) fn build_runtime_jobs_domain(
    runtime: &JobsRuntimeServices,
    module_jobs: &[RegisteredModuleJob],
    module_event_subscriptions: &[RegisteredEventSubscription],
) -> Result<
    (
        Vec<RuntimeJobDefinition>,
        Vec<RuntimeEventSubscriptionDefinition>,
        JobsDomain,
    ),
    RuntimeBuildError,
> {
    let mut jobs_by_name = BTreeMap::<String, RuntimeJobDefinition>::new();

    for registered in module_jobs {
        let queue = queue_for_job_trigger(runtime, registered.job.trigger);
        let retry_policy = retry_policy_for_job(runtime, &queue, &registered.job);
        let job = RuntimeJobDefinition {
            module: registered.module.clone(),
            contract: registered.job.clone(),
            queue,
            retry_policy,
        };

        if let Some(existing) = jobs_by_name.insert(job.contract.name.clone(), job.clone()) {
            return Err(RuntimeBuildError::DuplicateRuntimeJobName {
                job: job.contract.name,
                first_module: existing.module,
                second_module: job.module,
            });
        }
    }

    let mut domain = JobsDomain::new(runtime.clone());
    let mut subscriptions_by_handler = BTreeMap::<String, Vec<EventSubscriptionMetadata>>::new();
    let mut resolved_subscriptions = Vec::new();

    for registered in module_event_subscriptions {
        let Some(job_name) = registered.subscription.job.clone() else {
            return Err(RuntimeBuildError::EventSubscriptionMissingJob {
                module: registered.module.clone(),
                event: registered.subscription.event.clone(),
            });
        };
        let Some(job) = jobs_by_name.get(&job_name) else {
            return Err(RuntimeBuildError::UnknownEventSubscriptionJob {
                module: registered.module.clone(),
                event: registered.subscription.event.clone(),
                job: job_name,
            });
        };

        let event_type = DomainEventType::new(registered.subscription.event.clone())?;
        let subscription_id = EventSubscriptionId::new(format!(
            "{}:{}:{}",
            registered.module, registered.subscription.event, job.contract.name
        ))?;
        let handler_id = EventHandlerId::new(job.contract.name.clone())?;
        let reaction_queue = runtime.describe().domain_events_queue.clone();
        let reaction_retry_policy =
            retry_policy_for_contract_shape(runtime, &reaction_queue, job.contract.idempotent);
        let mut metadata = EventSubscriptionMetadata::new(
            subscription_id.clone(),
            event_type.clone(),
            reaction_queue.clone(),
            handler_id.clone(),
            reaction_retry_policy.clone(),
        );

        if reaction_retry_policy.is_retrying() {
            metadata = metadata.with_idempotency_key(IdempotencyKey::new(format!(
                "subscription:{}",
                subscription_id.as_str()
            ))?);
        }

        metadata = metadata.with_description(registered.subscription.description.clone())?;
        subscriptions_by_handler
            .entry(job.contract.name.clone())
            .or_default()
            .push(metadata.clone());
        resolved_subscriptions.push(RuntimeEventSubscriptionDefinition {
            module: registered.module.clone(),
            event_type,
            subscription_id,
            handler_id,
            job_name: job.contract.name.clone(),
            reaction_queue,
            retry_policy: reaction_retry_policy,
            target_trigger: job.contract.trigger,
            target_queue: job.queue.clone(),
            description: registered.subscription.description.clone(),
        });
        domain = domain.add_subscription(metadata);
    }

    let mut resolved_jobs = jobs_by_name.into_values().collect::<Vec<_>>();
    resolved_jobs.sort_by(|left, right| left.contract.name.cmp(&right.contract.name));
    resolved_subscriptions.sort_by(|left, right| left.subscription_id.cmp(&right.subscription_id));

    for (job_name, subscriptions) in &subscriptions_by_handler {
        let handler_id = EventHandlerId::new(job_name.clone())?;
        let mut handler = EventHandlerMetadata::new(
            handler_id,
            job_name.clone(),
            runtime.describe().domain_events_queue.clone(),
            RetryPolicy::default(),
        )?;

        for subscription in subscriptions {
            handler = handler.add_subscription(subscription.clone());
        }

        domain = domain.add_handler(handler);
    }

    domain.validate()?;

    Ok((resolved_jobs, resolved_subscriptions, domain))
}

fn queue_for_job_trigger(runtime: &JobsRuntimeServices, trigger: JobTriggerKind) -> JobQueueName {
    match trigger {
        JobTriggerKind::Scheduled => runtime.describe().scheduled_queue.clone(),
        JobTriggerKind::DomainEvent => runtime.describe().domain_events_queue.clone(),
        JobTriggerKind::Operator | JobTriggerKind::Webhook | JobTriggerKind::InlineFollowup => {
            runtime.describe().work_queue.clone()
        }
    }
}

fn retry_policy_for_job(
    runtime: &JobsRuntimeServices,
    queue: &JobQueueName,
    contract: &JobContract,
) -> RetryPolicy {
    retry_policy_for_contract_shape(runtime, queue, contract.idempotent)
}

fn retry_policy_for_contract_shape(
    runtime: &JobsRuntimeServices,
    queue: &JobQueueName,
    idempotent: bool,
) -> RetryPolicy {
    if idempotent {
        runtime
            .describe()
            .queue(queue)
            .map(|definition| definition.retry_policy.clone())
            .unwrap_or_default()
    } else {
        RetryPolicy::default()
    }
}

pub(crate) fn validate_runtime_identifier(
    field: &'static str,
    value: String,
) -> Result<String, RuntimeJobsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RuntimeJobsError::EmptyValue { field })
    } else {
        Ok(trimmed.to_string())
    }
}
