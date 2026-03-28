use super::super::*;
use super::types::{
    RegisteredEventSubscription, RegisteredModuleJob, RuntimeEventSubscriptionDefinition,
    RuntimeJobDefinition,
};
use std::collections::BTreeMap;

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
            coil_wasm::ExtensionPoint::Job(job) => JobContract::new(
                job.job_name.clone(),
                JobTriggerKind::Operator,
                false,
                format!(
                    "WASM extension job `{}` from `{}`",
                    handler.handler_id,
                    extension.manifest().id
                ),
            ),
            coil_wasm::ExtensionPoint::ScheduledJob(job) => JobContract::new(
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
