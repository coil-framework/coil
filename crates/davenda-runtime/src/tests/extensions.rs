use super::*;

#[test]
fn runtime_builder_registers_installed_extensions_against_declared_slots() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_installed_extension(installed_admin_widget_extension())
        .build()
        .unwrap();

    assert_eq!(plan.installed_extensions.len(), 1);
    assert_eq!(plan.installed_extensions[0].extension_id, "admin.waitlist");
    assert!(plan.registered_extension_slots.iter().any(|slot| {
        slot.module == "admin"
            && slot.kind == ExtensionPointKind::AdminWidget
            && slot.surface == "admin.dashboard.summary"
    }));
    assert_eq!(
        plan.extension_registry
            .admin_widget_handlers("admin.dashboard.summary")
            .len(),
        1
    );
}

#[test]
fn runtime_builder_rejects_extensions_without_declared_slots() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_installed_extension(installed_admin_widget_extension())
        .build()
        .unwrap_err();

    match error {
        RuntimeBuildError::UnknownExtensionSlot {
            extension_id,
            handler_id,
            point,
            surface,
        } => {
            assert_eq!(extension_id, "admin.waitlist");
            assert_eq!(handler_id, "waitlist-summary");
            assert_eq!(point, ExtensionPointKind::AdminWidget);
            assert_eq!(surface, "admin.dashboard.summary");
        }
        other => panic!("expected unknown extension slot, got {other:?}"),
    }
}

#[test]
fn wasm_host_prepares_admin_widget_invocations_from_request_execution() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(EventsModule::new())
        .with_installed_extension(installed_admin_widget_extension())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-operator-1")
        .unwrap();
    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/admin")
                .unwrap()
                .with_session_cookie(session_cookie)
                .with_principal("operator-1")
                .grant_capability(Capability::AdminShellAccess),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    let mut sessions = plan
        .wasm_host()
        .begin_admin_widget_invocations("admin.dashboard.summary", &execution)
        .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].plan().context.customer_app.locale.as_deref(),
        Some("en-GB")
    );
    assert_eq!(
        sessions[0].plan().context.customer_app.tenant_id.as_deref(),
        Some("101")
    );
    assert_eq!(
        sessions[0].plan().context.principal.kind,
        PrincipalKind::User
    );
    assert_eq!(
        sessions[0].plan().context.principal.id.as_deref(),
        Some("operator-1")
    );
    assert!(matches!(
        sessions[0].plan().context.input,
        InvocationInput::AdminWidget(_)
    ));

    sessions[0].record_host_call(HostCall::AuthCheck).unwrap();
    assert!(matches!(
        &sessions[0].host_service_executions()[0].result,
        davenda_wasm::HostServiceResult::Auth(davenda_wasm::AuthServiceExecution {
            details: davenda_wasm::AuthServiceDetails::Check { object, .. },
            ..
        }) if object == "tenant:101"
    ));
    sessions[0]
        .record_host_call(HostCall::DataRead {
            resource: "events.waitlist".to_string(),
        })
        .unwrap();
    assert!(matches!(
        &sessions[0].host_service_executions()[1].result,
        davenda_wasm::HostServiceResult::Data(davenda_wasm::DataServiceExecution {
            summary,
            ..
        }) if summary.contains("repository=events.waitlist")
            && summary.contains("synthetic=true")
    ));

    let receipt = sessions
        .into_iter()
        .next()
        .unwrap()
        .finish(
            Duration::from_millis(10),
            InvocationOutcome::AdminWidget,
            None,
        )
        .unwrap();

    assert_eq!(receipt.point, ExtensionPointKind::AdminWidget);
}

#[test]
fn wasm_host_executes_admin_widget_handlers_inside_the_engine() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(EventsModule::new())
        .with_installed_extension(installed_admin_widget_extension())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-operator-2")
        .unwrap();
    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/admin")
                .unwrap()
                .with_session_cookie(session_cookie)
                .with_principal("operator-2")
                .grant_capability(Capability::AdminShellAccess),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    let mut sessions = plan
        .wasm_host()
        .begin_admin_widget_invocations("admin.dashboard.summary", &execution)
        .unwrap();
    let session = sessions.remove(0);
    let slots = session.grant_slots();
    let data_slot = slots
        .iter()
        .position(|grant| {
            grant
                == &HostCapabilityGrant::DataRead {
                    resource: "events.waitlist".to_string(),
                }
        })
        .unwrap() as i32;
    let auth_slot = slots
        .iter()
        .position(|grant| grant == &HostCapabilityGrant::AuthCheck)
        .unwrap() as i32;

    let host = plan.wasm_host();
    let module = host
        .compile_module(
            guest_module(
                "exports.waitlist_summary",
                &[(data_slot, 0), (auth_slot, 0)],
                InvocationOutcome::AdminWidget,
            )
            .as_bytes(),
        )
        .unwrap();

    let receipt = host.execute_session(&module, session).unwrap();
    assert_eq!(receipt.outcome, InvocationOutcome::AdminWidget);
    assert_eq!(receipt.point, ExtensionPointKind::AdminWidget);
}

#[test]
fn wasm_host_prepares_render_hook_invocations_from_request_execution() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_installed_extension(installed_render_hook_extension())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/en-GB/pages/home").unwrap(),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    let mut sessions = plan
        .wasm_host()
        .begin_render_hook_invocations("cms.page.render", &execution)
        .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].plan().context.customer_app.locale.as_deref(),
        Some("en-GB")
    );
    assert_eq!(
        sessions[0].plan().context.principal.kind,
        PrincipalKind::Anonymous
    );
    assert!(matches!(
        sessions[0].plan().context.input,
        InvocationInput::RenderHook(_)
    ));

    sessions[0]
        .record_host_call(HostCall::RenderFragment {
            slot: "cms.page.render".to_string(),
        })
        .unwrap();

    let receipt = sessions
        .into_iter()
        .next()
        .unwrap()
        .finish(
            Duration::from_millis(8),
            InvocationOutcome::RenderHook,
            None,
        )
        .unwrap();

    assert_eq!(receipt.point, ExtensionPointKind::RenderHook);
}

#[test]
fn wasm_host_preserves_service_account_principal_kind_from_request_execution() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_installed_extension(installed_render_hook_extension())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/en-GB/pages/home")
                .unwrap()
                .with_service_account_principal("runtime.service-hooks"),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    let sessions = plan
        .wasm_host()
        .begin_render_hook_invocations("cms.page.render", &execution)
        .unwrap();

    assert_eq!(sessions.len(), 1);
    assert_eq!(
        sessions[0].plan().context.principal.kind,
        PrincipalKind::ServiceAccount
    );
    assert_eq!(
        sessions[0].plan().context.principal.id.as_deref(),
        Some("runtime.service-hooks")
    );
}

#[test]
fn wasm_host_prepares_job_and_webhook_invocations() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(CommerceModule::new())
        .with_module(OpsModule::new())
        .with_installed_extension(installed_job_extension())
        .with_installed_extension(installed_webhook_extension())
        .build()
        .unwrap();

    let mut job_session = plan
        .wasm_host()
        .begin_job_invocation(
            "ops.search.adapter",
            2,
            "trace.jobs.search-adapter",
            ExtensionPrincipal::service_account("ops.search-worker"),
        )
        .unwrap()
        .expect("job extension is installed");

    assert_eq!(
        job_session.plan().context.principal.kind,
        PrincipalKind::ServiceAccount
    );
    assert_eq!(
        job_session.plan().context.principal.id.as_deref(),
        Some("ops.search-worker")
    );
    assert!(matches!(
        job_session.plan().context.input,
        InvocationInput::Job(_)
    ));
    job_session
        .record_host_call(HostCall::EnqueueJob {
            queue: "jobs.work".to_string(),
        })
        .unwrap();
    let job_receipt = job_session
        .finish(
            Duration::from_millis(25),
            InvocationOutcome::JobCompleted,
            None,
        )
        .unwrap();
    assert_eq!(job_receipt.point, ExtensionPointKind::Job);

    let mut webhook_session = plan
        .wasm_host()
        .begin_webhook_invocation(
            "commerce.payment-provider",
            "payment.authorized",
            true,
            true,
            "trace.webhooks.payment-authorized",
            ExtensionPrincipal::service_account("commerce.webhooks"),
        )
        .unwrap()
        .expect("webhook extension is installed");

    assert_eq!(
        webhook_session.plan().context.principal.kind,
        PrincipalKind::ServiceAccount
    );
    assert!(matches!(
        webhook_session.plan().context.input,
        InvocationInput::Webhook(_)
    ));
    webhook_session
        .record_host_call(HostCall::EnqueueJob {
            queue: "jobs.work".to_string(),
        })
        .unwrap();
    let webhook_receipt = webhook_session
        .finish(
            Duration::from_millis(15),
            InvocationOutcome::WebhookAccepted,
            None,
        )
        .unwrap();
    assert_eq!(webhook_receipt.point, ExtensionPointKind::Webhook);
}

#[test]
fn wasm_host_prepares_leased_job_invocations_for_extension_jobs() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_installed_extension(installed_job_extension())
        .build()
        .unwrap();

    assert!(plan.registered_runtime_jobs.iter().any(|job| {
        job.module == "extension:ops.search.worker" && job.contract.name == "ops.search.adapter"
    }));

    let mut jobs = plan.jobs_host("scheduler-a").unwrap();
    let now = JobInstant::from_unix_seconds(50);
    jobs.enqueue_job(
        JobDispatchRequest::new("ops.search.adapter", "rebuild search indexes").unwrap(),
        now,
    )
    .unwrap();

    let work_queue = jobs.queue_topology.work_queue.clone();
    let leases = jobs
        .lease_ready_jobs(
            &work_queue,
            "worker-1",
            JobInstant::from_unix_seconds(51),
            Duration::from_secs(30),
            1,
        )
        .unwrap();
    let mut session = plan
        .wasm_host()
        .begin_leased_job_invocation(&leases[0])
        .unwrap()
        .expect("leased extension job should be executable");

    assert_eq!(session.plan().point, ExtensionPointKind::Job);
    assert_eq!(
        session.plan().context.principal.kind,
        PrincipalKind::ServiceAccount
    );
    assert_eq!(
        session.plan().context.principal.id.as_deref(),
        Some("runtime.jobs")
    );
    assert!(matches!(
        session.plan().context.input,
        InvocationInput::Job(ref job)
            if job.job_name == "ops.search.adapter" && job.attempt == 1
    ));

    session
        .record_host_call(HostCall::EnqueueJob {
            queue: "jobs.work".to_string(),
        })
        .unwrap();
    let receipt = session
        .finish(
            Duration::from_millis(12),
            InvocationOutcome::JobCompleted,
            None,
        )
        .unwrap();
    assert_eq!(receipt.extension_id.to_string(), "ops.search.worker");
}

#[test]
fn wasm_host_prepares_leased_scheduled_job_invocations_for_extensions() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let scheduled_slots =
        StaticManifestModule::new(ModuleManifest::new("scheduler.slots").with_extension_slots(
            vec![ExtensionSlotDescriptor::new(
                ExtensionSlotKind::ScheduledJob,
                "ops.search.nightly",
                "Allows scheduled search extension workflows",
            )],
        ));
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(scheduled_slots)
        .with_installed_extension(installed_scheduled_job_extension())
        .build()
        .unwrap();

    assert!(plan.registered_runtime_jobs.iter().any(|job| {
        job.module == "extension:ops.search.nightly"
            && job.contract.name == "ops.search.nightly"
            && job.contract.trigger == JobTriggerKind::Scheduled
    }));

    let mut jobs = plan.jobs_host("scheduler-a").unwrap();
    let scheduled_for = JobInstant::from_unix_seconds(100);
    jobs.enqueue_job(
        JobDispatchRequest::new("ops.search.nightly", "nightly search refresh")
            .unwrap()
            .scheduled_for(scheduled_for),
        JobInstant::from_unix_seconds(10),
    )
    .unwrap();
    jobs.acquire_scheduler_leadership(JobInstant::from_unix_seconds(150), Duration::from_secs(30))
        .unwrap();
    jobs.promote_due_jobs(JobInstant::from_unix_seconds(150))
        .unwrap();

    let scheduled_queue = jobs.queue_topology.scheduled_queue.clone();
    let leases = jobs
        .lease_ready_jobs(
            &scheduled_queue,
            "worker-1",
            JobInstant::from_unix_seconds(151),
            Duration::from_secs(30),
            1,
        )
        .unwrap();
    let mut session = plan
        .wasm_host()
        .begin_leased_job_invocation(&leases[0])
        .unwrap()
        .expect("leased scheduled extension job should be executable");

    assert_eq!(session.plan().point, ExtensionPointKind::ScheduledJob);
    assert!(matches!(
        session.plan().context.input,
        InvocationInput::ScheduledJob(ref job) if job.job_name == "ops.search.nightly"
    ));
    session
        .record_host_call(HostCall::EnqueueJob {
            queue: "jobs.work".to_string(),
        })
        .unwrap();
    let receipt = session
        .finish(
            Duration::from_millis(20),
            InvocationOutcome::ScheduledJobCompleted,
            None,
        )
        .unwrap();
    assert_eq!(receipt.extension_id.to_string(), "ops.search.nightly");
}

#[test]
fn wasm_host_rejects_unverified_webhook_execution() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_installed_extension(installed_webhook_extension())
        .build()
        .unwrap();

    let error = plan
        .wasm_host()
        .prepare_webhook_invocation(
            "commerce.payment-provider",
            "payment.authorized",
            false,
            true,
            "trace.webhooks.unverified",
            ExtensionPrincipal::service_account("commerce.webhooks"),
        )
        .unwrap_err();

    assert_eq!(
        error,
        WasmModelError::UnverifiedWebhook {
            handler_id: "payment-authorized".to_string(),
        }
    );
    let snapshot = plan.wasm_host().webhook_observation_snapshot(10).unwrap();
    assert!(snapshot.status_counts.verification_failed >= 1);
    assert!(!snapshot.recent_events.is_empty());
    assert_eq!(
        snapshot.recent_events[0].source,
        "commerce.payment-provider".to_string()
    );
    assert_eq!(
        snapshot.recent_events[0].event,
        "payment.authorized".to_string()
    );
    assert_eq!(
        snapshot.recent_events[0].status,
        WebhookObservationStatus::VerificationFailed
    );
}
