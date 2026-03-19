use super::*;
use davenda_config::JobBackend;
use std::time::Duration;

fn config(backend: JobBackend) -> davenda_config::JobsConfig {
    davenda_config::JobsConfig {
        backend,
        retry_limit: 3,
    }
}

#[test]
fn runtime_describes_backend_and_queue_topology() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let topology = runtime.describe();

    assert_eq!(runtime.backend, JobBackend::Redis);
    assert_eq!(topology.backend, JobBackend::Redis);
    assert_eq!(topology.work_queue.as_str(), "jobs.work");
    assert_eq!(topology.scheduled_queue.as_str(), "jobs.scheduled");
    assert_eq!(topology.domain_events_queue.as_str(), "jobs.domain-events");
    assert_eq!(topology.dead_letter_queue.as_str(), "jobs.dead-letter");
}

#[test]
fn planner_rejects_retrying_jobs_without_idempotency_keys() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Valkey)).unwrap();
    let planner = runtime.planner();
    let spec = JobSpec::new(
        JobId::new("job-1").unwrap(),
        JobName::new("reconcile-invoice").unwrap(),
        JobQueueName::new("jobs.work").unwrap(),
        "reconcile invoice state",
    )
    .unwrap()
    .with_retry_policy(
        RetryPolicy::new(3, Duration::from_secs(5), Duration::from_secs(60)).unwrap(),
    );

    let err = planner.validate_retry_safety(&spec).unwrap_err();
    assert!(matches!(
        err,
        JobsModelError::MissingIdempotencyKey { job_id } if job_id == "job-1"
    ));
}

#[test]
fn planner_accepts_scheduled_retry_safe_jobs() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let planner = runtime.planner();
    let spec = JobSpec::new(
        JobId::new("job-2").unwrap(),
        JobName::new("sitemap-regeneration").unwrap(),
        JobQueueName::new("jobs.scheduled").unwrap(),
        "regenerate sitemaps",
    )
    .unwrap()
    .scheduled_for(JobInstant::from_unix_seconds(10))
    .with_retry_policy(
        RetryPolicy::new(4, Duration::from_secs(5), Duration::from_secs(30))
            .unwrap()
            .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
    )
    .with_idempotency_key(IdempotencyKey::new("sitemap:v1").unwrap());

    let planned = planner
        .plan_job(spec, JobInstant::from_unix_seconds(5))
        .unwrap();
    assert_eq!(planned.queue.as_str(), "jobs.scheduled");
    assert!(matches!(
        planned.dead_letter_outcome,
        DeadLetterOutcomeKind::RouteToQueue(ref queue) if queue.as_str() == "jobs.dead-letter"
    ));
}

#[test]
fn planner_rejects_scheduled_jobs_in_the_past() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let planner = runtime.planner();
    let spec = JobSpec::new(
        JobId::new("job-3").unwrap(),
        JobName::new("cache-warm").unwrap(),
        JobQueueName::new("jobs.scheduled").unwrap(),
        "warm cache",
    )
    .unwrap()
    .scheduled_for(JobInstant::from_unix_seconds(5))
    .with_idempotency_key(IdempotencyKey::new("cache:warm:v1").unwrap());

    let err = planner
        .plan_job(spec, JobInstant::from_unix_seconds(5))
        .unwrap_err();
    assert!(matches!(
        err,
        JobsModelError::ScheduledInPast { scheduled_at, now }
            if scheduled_at.as_unix_seconds() == 5 && now.as_unix_seconds() == 5
    ));
}

#[test]
fn domain_event_metadata_tracks_subscriptions_and_handlers() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Valkey)).unwrap();
    let subscription = EventSubscriptionMetadata::new(
        EventSubscriptionId::new("sub-1").unwrap(),
        DomainEventType::new("order.completed").unwrap(),
        runtime.describe().domain_events_queue.clone(),
        EventHandlerId::new("handler-1").unwrap(),
        RetryPolicy::new(2, Duration::from_secs(10), Duration::from_secs(120))
            .unwrap()
            .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
    )
    .with_idempotency_key(IdempotencyKey::new("event:order.completed:v1").unwrap())
    .with_description("enqueue fulfillment work")
    .unwrap();

    let handler = EventHandlerMetadata::new(
        EventHandlerId::new("handler-1").unwrap(),
        "Fulfillment handler",
        runtime.describe().domain_events_queue.clone(),
        RetryPolicy::default(),
    )
    .unwrap()
    .add_subscription(subscription.clone());

    let domain = JobsDomain::new(runtime)
        .add_subscription(subscription)
        .add_handler(handler);

    assert!(domain.validate().is_ok());
}

#[test]
fn domain_event_envelopes_capture_correlation_and_causation() {
    let envelope = DomainEventEnvelope::new(
        DomainEventId::new("evt-1").unwrap(),
        DomainEventType::new("booking.created").unwrap(),
        "booking",
        "booking-42",
        JobInstant::from_unix_seconds(100),
        "payload".to_string(),
    )
    .unwrap()
    .with_correlation_id("corr-1")
    .unwrap()
    .with_causation_id("cause-1")
    .unwrap();

    assert_eq!(envelope.correlation_id.as_deref(), Some("corr-1"));
    assert_eq!(envelope.causation_id.as_deref(), Some("cause-1"));
    assert_eq!(envelope.version, 1);
}

#[test]
fn scheduler_leadership_promotes_due_jobs_once() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let mut coordinator = runtime.coordinator();
    coordinator
        .enqueue(
            JobSpec::new(
                JobId::new("job-scheduled").unwrap(),
                JobName::new("sitemap-regeneration").unwrap(),
                runtime.describe().scheduled_queue.clone(),
                "regenerate sitemap",
            )
            .unwrap()
            .scheduled_for(JobInstant::from_unix_seconds(20))
            .with_retry_policy(
                RetryPolicy::new(2, Duration::from_secs(10), Duration::from_secs(30))
                    .unwrap()
                    .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
            )
            .with_idempotency_key(IdempotencyKey::new("sitemap-regeneration").unwrap()),
            JobInstant::from_unix_seconds(10),
        )
        .unwrap();

    let leader = coordinator
        .acquire_scheduler_leadership(
            "node-a",
            JobInstant::from_unix_seconds(12),
            Duration::from_secs(30),
        )
        .unwrap();
    assert_eq!(leader.node_id, "node-a");
    assert_eq!(coordinator.ready_jobs().len(), 0);
    assert_eq!(coordinator.scheduled_jobs().len(), 1);

    let promoted = coordinator
        .promote_due_jobs("node-a", JobInstant::from_unix_seconds(20))
        .unwrap();
    assert_eq!(promoted, vec![JobId::new("job-scheduled").unwrap()]);
    assert_eq!(coordinator.ready_jobs().len(), 1);
    assert_eq!(coordinator.scheduled_jobs().len(), 0);

    let err = coordinator
        .acquire_scheduler_leadership(
            "node-b",
            JobInstant::from_unix_seconds(25),
            Duration::from_secs(30),
        )
        .unwrap_err();
    assert!(matches!(
        err,
        JobsModelError::LeadershipConflict {
            current_holder,
            requested_holder,
        } if current_holder == "node-a" && requested_holder == "node-b"
    ));
}

#[test]
fn failed_jobs_retry_then_dead_letter_after_exhaustion() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Valkey)).unwrap();
    let mut coordinator = runtime.coordinator();
    coordinator
        .enqueue(
            JobSpec::new(
                JobId::new("job-retry").unwrap(),
                JobName::new("certificate-renewal").unwrap(),
                runtime.describe().work_queue.clone(),
                "renew certificate",
            )
            .unwrap()
            .with_retry_policy(
                RetryPolicy::new(2, Duration::from_secs(15), Duration::from_secs(60))
                    .unwrap()
                    .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
            )
            .with_idempotency_key(IdempotencyKey::new("certificate-renewal:v1").unwrap()),
            JobInstant::from_unix_seconds(100),
        )
        .unwrap();

    let first_lease = coordinator
        .lease_ready_jobs(
            &runtime.describe().work_queue,
            "worker-a",
            JobInstant::from_unix_seconds(100),
            Duration::from_secs(30),
            1,
        )
        .unwrap()
        .remove(0);

    let retry = coordinator
        .acknowledge_failed(
            &first_lease,
            JobInstant::from_unix_seconds(105),
            DeadLetterReason::PolicyViolation,
            "temporary upstream failure",
        )
        .unwrap();
    assert!(matches!(
        retry,
        JobFailureDisposition::Retried { ref queue, .. } if queue.as_str() == "jobs.work"
    ));
    assert_eq!(coordinator.ready_jobs().len(), 0);
    assert_eq!(coordinator.scheduled_jobs().len(), 1);

    coordinator
        .acquire_scheduler_leadership(
            "node-a",
            JobInstant::from_unix_seconds(110),
            Duration::from_secs(60),
        )
        .unwrap();
    coordinator
        .promote_due_jobs("node-a", JobInstant::from_unix_seconds(120))
        .unwrap();

    let second_lease = coordinator
        .lease_ready_jobs(
            &runtime.describe().work_queue,
            "worker-a",
            JobInstant::from_unix_seconds(120),
            Duration::from_secs(30),
            1,
        )
        .unwrap()
        .remove(0);
    let dead_letter = coordinator
        .acknowledge_failed(
            &second_lease,
            JobInstant::from_unix_seconds(125),
            DeadLetterReason::ExhaustedRetries,
            "permanent failure",
        )
        .unwrap();
    assert!(matches!(
        dead_letter,
        JobFailureDisposition::DeadLettered(_)
    ));
    assert_eq!(coordinator.dead_letters().len(), 1);
    assert_eq!(coordinator.dead_letters()[0].job_id.as_str(), "job-retry");
}

#[test]
fn domain_events_dispatch_into_subscription_jobs() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let subscription = EventSubscriptionMetadata::new(
        EventSubscriptionId::new("sub-booking-email").unwrap(),
        DomainEventType::new("booking.confirmed").unwrap(),
        runtime.describe().domain_events_queue.clone(),
        EventHandlerId::new("handler-booking-email").unwrap(),
        RetryPolicy::new(2, Duration::from_secs(5), Duration::from_secs(30))
            .unwrap()
            .with_dead_letter_queue(runtime.describe().dead_letter_queue.clone()),
    )
    .with_idempotency_key(IdempotencyKey::new("booking.confirmed.email").unwrap());
    let handler = EventHandlerMetadata::new(
        EventHandlerId::new("handler-booking-email").unwrap(),
        "Booking email",
        runtime.describe().domain_events_queue.clone(),
        RetryPolicy::default(),
    )
    .unwrap()
    .add_subscription(subscription.clone());
    let domain = JobsDomain::new(runtime.clone())
        .add_subscription(subscription)
        .add_handler(handler);

    let envelope = DomainEventEnvelope::new(
        DomainEventId::new("evt-booking-1").unwrap(),
        DomainEventType::new("booking.confirmed").unwrap(),
        "booking",
        "booking-1",
        JobInstant::from_unix_seconds(200),
        (),
    )
    .unwrap();

    let mut coordinator = runtime.coordinator();
    let job_ids = coordinator
        .dispatch_event(&domain, &envelope, JobInstant::from_unix_seconds(200))
        .unwrap();
    assert_eq!(
        job_ids,
        vec![JobId::new("event:evt-booking-1:sub-booking-email").unwrap()]
    );
    assert_eq!(coordinator.ready_jobs().len(), 1);
    assert_eq!(
        coordinator.ready_jobs()[0].spec.queue,
        runtime.describe().domain_events_queue
    );
}

#[test]
fn distributed_coordinators_do_not_share_backend_without_explicit_adapter_reuse() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let mut left = runtime.coordinator_for_testing();
    let mut right = runtime.coordinator_for_testing();

    left.enqueue(
        JobSpec::new(
            JobId::new("job-shared").unwrap(),
            JobName::new("shared-work").unwrap(),
            runtime.describe().work_queue.clone(),
            "shared backend work item",
        )
        .unwrap()
        .with_idempotency_key(IdempotencyKey::new("shared-work:v1").unwrap()),
        JobInstant::from_unix_seconds(10),
    )
    .unwrap();

    right.refresh();
    assert_eq!(right.ready_jobs().len(), 0);

    left.refresh();
    assert_eq!(left.ready_jobs().len(), 1);
    assert_eq!(left.ready_jobs()[0].spec.job_id.as_str(), "job-shared");
}

#[test]
fn default_coordinators_are_local_even_for_distributed_topologies() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let mut left = runtime.coordinator();
    let mut right = runtime.coordinator();

    left.enqueue(
        JobSpec::new(
            JobId::new("job-shared").unwrap(),
            JobName::new("shared-work").unwrap(),
            runtime.describe().work_queue.clone(),
            "shared backend work item",
        )
        .unwrap()
        .with_idempotency_key(IdempotencyKey::new("shared-work:v1").unwrap()),
        JobInstant::from_unix_seconds(10),
    )
    .unwrap();

    right.refresh();
    assert_eq!(right.ready_jobs().len(), 0);
}

#[test]
fn distributed_coordinators_share_backend_when_using_an_explicit_shared_runtime() {
    let runtime = JobsRuntime::from_config(&config(JobBackend::Redis)).unwrap();
    let shared_runtime = JobsBackendAdapter::emulated_shared_runtime(&runtime);
    let mut left = runtime.coordinator_with_shared_runtime(shared_runtime.clone());
    let mut right = runtime.coordinator_with_shared_runtime(shared_runtime);

    left.enqueue(
        JobSpec::new(
            JobId::new("job-shared").unwrap(),
            JobName::new("shared-work").unwrap(),
            runtime.describe().work_queue.clone(),
            "shared backend work item",
        )
        .unwrap()
        .with_idempotency_key(IdempotencyKey::new("shared-work:v1").unwrap()),
        JobInstant::from_unix_seconds(10),
    )
    .unwrap();

    right.refresh();
    assert_eq!(right.ready_jobs().len(), 1);
    assert_eq!(right.ready_jobs()[0].spec.job_id.as_str(), "job-shared");

    let leased = right
        .lease_ready_jobs(
            &runtime.describe().work_queue,
            "worker-shared",
            JobInstant::from_unix_seconds(10),
            Duration::from_secs(30),
            1,
        )
        .unwrap();
    assert_eq!(leased.len(), 1);

    left.refresh();
    assert_eq!(left.ready_jobs().len(), 0);
    assert_eq!(left.in_flight_jobs().len(), 1);
}
