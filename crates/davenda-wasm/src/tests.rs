use super::*;
use std::collections::BTreeMap;
use std::time::Duration;

fn default_limits() -> ResourceLimits {
    ResourceLimits::baseline_for(ExtensionPointKind::Page)
}

fn guest_module(export: &str, host_calls: &[(i32, i64)], outcome: InvocationOutcome) -> String {
    let mut body = String::new();
    for (slot, metric) in host_calls {
        body.push_str(&format!(
            "    i32.const {slot}\n    i64.const {metric}\n    call $host_call\n    drop\n"
        ));
    }

    format!(
        "(module
            (import \"davenda\" \"host_call\" (func $host_call (param i32 i64) (result i32)))
            (func (export \"{export}\") (result i32)
{body}                i32.const {}
            )
        )",
        outcome.engine_code()
    )
}

fn page_manifest() -> ExtensionManifest {
    let page_handler = HandlerManifest::new(
        HandlerId::new("waitlist-page").unwrap(),
        "exports.page_waitlist",
        ExtensionPoint::Page(
            PageExtensionPoint::new("/events/waitlist", [HttpMethod::Get, HttpMethod::Post])
                .unwrap(),
        ),
        HostGrantSet::from_grants([
            HostCapabilityGrant::DataRead {
                resource: "events.waitlist".to_string(),
            },
            HostCapabilityGrant::AuthCheck,
            HostCapabilityGrant::RenderFragment {
                slot: "events.waitlist.panel".to_string(),
            },
            HostCapabilityGrant::CacheHintWrite,
        ]),
    )
    .unwrap();

    ExtensionManifest::new(
        ExtensionId::new("events.waitlist").unwrap(),
        "Events Waitlist Tools",
        ContractVersion::new(1, 0, 0),
        ContractVersion::new(1, 0, 0),
        default_limits(),
        vec![page_handler],
    )
    .unwrap()
}

#[test]
fn manifest_rejects_visual_render_grants_on_job_handlers() {
    let handler = HandlerManifest::new(
        HandlerId::new("reconcile-job").unwrap(),
        "exports.reconcile",
        ExtensionPoint::Job(JobExtensionPoint::new("reconcile", "default").unwrap()),
        HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
            slot: "admin.dashboard".to_string(),
        }]),
    )
    .unwrap();

    let error = ExtensionManifest::new(
        ExtensionId::new("jobs.reconcile").unwrap(),
        "Reconcile Jobs",
        ContractVersion::new(1, 0, 0),
        ContractVersion::new(1, 0, 0),
        ResourceLimits::baseline_for(ExtensionPointKind::Job),
        vec![handler],
    )
    .unwrap_err();

    assert_eq!(
        error,
        WasmModelError::UnsupportedGrantForPoint {
            handler_id: "reconcile-job".to_string(),
            point: ExtensionPointKind::Job,
            grant: HostCapabilityGrant::RenderFragment {
                slot: "admin.dashboard".to_string(),
            },
        }
    );
}

#[test]
fn installation_rejects_grants_that_were_not_declared() {
    let manifest = page_manifest();
    let installation = ExtensionInstallation::new(
        "customer-app",
        vec![HandlerInstallation::new(
            HandlerId::new("waitlist-page").unwrap(),
            HostGrantSet::from_grants([
                HostCapabilityGrant::AuthCheck,
                HostCapabilityGrant::SecretRead {
                    secret: "undocumented".to_string(),
                },
            ]),
        )],
    )
    .unwrap();

    let error = InstalledExtension::install(manifest, installation).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::GrantNotDeclared {
            handler_id: "waitlist-page".to_string(),
            grant: HostCapabilityGrant::SecretRead {
                secret: "undocumented".to_string(),
            },
        }
    );
}

#[test]
fn extension_package_validates_configuration_schema_and_installation() {
    let manifest = ExtensionManifest::new(
        ExtensionId::new("loyalty.widget").unwrap(),
        "Loyalty Widget",
        ContractVersion::new(1, 2, 3),
        ContractVersion::new(1, 0, 0),
        ResourceLimits::baseline_for(ExtensionPointKind::RenderHook),
        vec![
            HandlerManifest::new(
                HandlerId::new("account.loyalty.widget").unwrap(),
                "exports.loyalty_widget",
                ExtensionPoint::RenderHook(
                    RenderHookExtensionPoint::new("cms.page.render").unwrap(),
                ),
                HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                    slot: "cms.page.render".to_string(),
                }]),
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let package = ExtensionPackage::new(
        "worka",
        manifest,
        ExtensionArtifactSource::local_path("extensions/loyalty-widget.wasm").unwrap(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ExtensionConfigSchema::new(
            1,
            vec![
                ExtensionConfigField::required("program_slug", ExtensionConfigValueType::String)
                    .unwrap(),
                ExtensionConfigField::optional("show_points", ExtensionConfigValueType::Boolean)
                    .unwrap()
                    .with_default(ExtensionConfigValue::Boolean(true))
                    .unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();

    let installed = package
        .install(
            ExtensionInstallation::new(
                "customer-app",
                vec![HandlerInstallation::new(
                    HandlerId::new("account.loyalty.widget").unwrap(),
                    HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                        slot: "cms.page.render".to_string(),
                    }]),
                )],
            )
            .unwrap(),
            &BTreeMap::from([(
                "program_slug".to_string(),
                ExtensionConfigValue::String("harbor-club".to_string()),
            )]),
        )
        .unwrap();

    assert_eq!(installed.config().len(), 2);
    assert_eq!(installed.manifest().version, ContractVersion::new(1, 2, 3));
}

#[test]
fn installed_extension_prepares_invocation_with_granted_capabilities_and_limits() {
    let manifest = page_manifest();
    let installed = InstalledExtension::install(
        manifest,
        ExtensionInstallation::new(
            "customer-app",
            vec![
                HandlerInstallation::new(
                    HandlerId::new("waitlist-page").unwrap(),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::AuthCheck,
                        HostCapabilityGrant::DataRead {
                            resource: "events.waitlist".to_string(),
                        },
                    ]),
                )
                .with_limit_override(ResourceLimits::new(
                    Duration::from_secs(1),
                    32 * 1024 * 1024,
                    2,
                    2 * 1024 * 1024,
                    1,
                    2 * 1024 * 1024,
                    8,
                )),
            ],
        )
        .unwrap(),
    )
    .unwrap();

    let plan = installed
        .prepare_invocation(
            &HandlerId::new("waitlist-page").unwrap(),
            InvocationContext::new(
                CustomerAppContext::new("customer-app")
                    .unwrap()
                    .with_site_id("main-site")
                    .unwrap()
                    .with_locale("en-GB")
                    .unwrap(),
                PrincipalRef::user("user-42").unwrap(),
                TraceContext::new("trace-123")
                    .unwrap()
                    .with_request_id("req-99")
                    .unwrap(),
                InvocationInput::Page(
                    PageInvocation::new("/events/waitlist", HttpMethod::Post).unwrap(),
                ),
            ),
        )
        .unwrap();

    assert_eq!(plan.point, ExtensionPointKind::Page);
    assert_eq!(plan.customer_app_id, "customer-app");
    assert_eq!(plan.granted_capabilities.len(), 2);
    assert_eq!(plan.limits.max_runtime, Duration::from_secs(1));
    assert_eq!(plan.context.customer_app.locale.as_deref(), Some("en-GB"));
}

#[test]
fn execution_session_enforces_host_grants_and_resource_limits() {
    let manifest = ExtensionManifest::new(
        ExtensionId::new("events.waitlist.exec").unwrap(),
        "Events Waitlist Execution",
        ContractVersion::new(1, 0, 0),
        ContractVersion::new(1, 0, 0),
        default_limits(),
        vec![
            HandlerManifest::new(
                HandlerId::new("waitlist-page").unwrap(),
                "exports.page_waitlist",
                ExtensionPoint::Page(
                    PageExtensionPoint::new(
                        "/events/waitlist",
                        [HttpMethod::Get, HttpMethod::Post],
                    )
                    .unwrap(),
                ),
                HostGrantSet::from_grants([
                    HostCapabilityGrant::AuthCheck,
                    HostCapabilityGrant::OutboundHttp {
                        integration: "crm".to_string(),
                    },
                    HostCapabilityGrant::StorageWrite {
                        class: StorageClassGrant::PrivateShared,
                    },
                ]),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let plan = InstalledExtension::install(
        manifest,
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page").unwrap(),
                HostGrantSet::from_grants([
                    HostCapabilityGrant::AuthCheck,
                    HostCapabilityGrant::OutboundHttp {
                        integration: "crm".to_string(),
                    },
                    HostCapabilityGrant::StorageWrite {
                        class: StorageClassGrant::PrivateShared,
                    },
                ]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
    .prepare_invocation(
        &HandlerId::new("waitlist-page").unwrap(),
        InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-42").unwrap(),
            TraceContext::new("trace-1").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let mut session = plan.begin_execution();
    session.record_host_call(HostCall::AuthCheck).unwrap();
    session
        .record_host_call(HostCall::OutboundHttp {
            integration: "crm".to_string(),
            response_bytes: 512,
        })
        .unwrap();
    session
        .record_host_call(HostCall::StorageWrite {
            class: StorageClassGrant::PrivateShared,
            bytes: 1_024,
        })
        .unwrap();
    let denied = session
        .record_host_call(HostCall::SecretRead {
            secret: "tls-account".to_string(),
        })
        .unwrap_err();
    assert_eq!(
        denied,
        WasmModelError::HostGrantDenied {
            handler_id: "waitlist-page".to_string(),
            grant: HostCapabilityGrant::SecretRead {
                secret: "tls-account".to_string(),
            },
        }
    );
}

#[test]
fn execution_session_rejects_invalid_outcomes_and_runtime_overruns() {
    let manifest = ExtensionManifest::new(
        ExtensionId::new("jobs.reconcile").unwrap(),
        "Reconcile Jobs",
        ContractVersion::new(1, 0, 0),
        ContractVersion::new(1, 0, 0),
        ResourceLimits::baseline_for(ExtensionPointKind::Job),
        vec![
            HandlerManifest::new(
                HandlerId::new("reconcile-job").unwrap(),
                "exports.reconcile",
                ExtensionPoint::Job(JobExtensionPoint::new("reconcile", "jobs.work").unwrap()),
                HostGrantSet::from_grants([HostCapabilityGrant::DataWrite {
                    resource: "billing.invoice".to_string(),
                }]),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let plan = InstalledExtension::install(
        manifest,
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("reconcile-job").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::DataWrite {
                    resource: "billing.invoice".to_string(),
                }]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
    .prepare_invocation(
        &HandlerId::new("reconcile-job").unwrap(),
        InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::service_account("svc-jobs").unwrap(),
            TraceContext::new("trace-job").unwrap(),
            InvocationInput::Job(JobInvocation::new("reconcile", 1).unwrap()),
        ),
    )
    .unwrap();

    let invalid = plan
        .clone()
        .begin_execution()
        .finish(Duration::from_secs(1), InvocationOutcome::ApiJson)
        .unwrap_err();
    assert_eq!(
        invalid,
        WasmModelError::InvalidOutcomeForPoint {
            handler_id: "reconcile-job".to_string(),
            point: ExtensionPointKind::Job,
            outcome: "api_json",
        }
    );

    let over_budget = plan
        .begin_execution()
        .finish(Duration::from_secs(31), InvocationOutcome::JobCompleted)
        .unwrap_err();
    assert!(matches!(
        over_budget,
        WasmModelError::RuntimeBudgetExceeded { .. }
    ));
}

#[test]
fn wasm_engine_executes_guest_handlers_against_granted_slots() {
    let plan = InstalledExtension::install(
        page_manifest(),
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page").unwrap(),
                HostGrantSet::from_grants([
                    HostCapabilityGrant::AuthCheck,
                    HostCapabilityGrant::DataRead {
                        resource: "events.waitlist".to_string(),
                    },
                ]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
    .prepare_invocation(
        &HandlerId::new("waitlist-page").unwrap(),
        InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-99").unwrap(),
            TraceContext::new("trace-engine").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let slots = plan.grant_slots();
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

    let engine = WasmEngine::new();
    let module = engine
        .compile_module(
            guest_module(
                "exports.page_waitlist",
                &[(data_slot, 0), (auth_slot, 0)],
                InvocationOutcome::Page,
            )
            .as_bytes(),
        )
        .unwrap();

    let receipt = engine
        .execute_session(&module, plan.begin_execution(), "exports.page_waitlist")
        .unwrap();
    assert_eq!(receipt.outcome, InvocationOutcome::Page);
    assert_eq!(receipt.point, ExtensionPointKind::Page);
}

#[test]
fn wasm_engine_rejects_invalid_capability_slots() {
    let plan = InstalledExtension::install(
        page_manifest(),
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
    .prepare_invocation(
        &HandlerId::new("waitlist-page").unwrap(),
        InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-5").unwrap(),
            TraceContext::new("trace-invalid-slot").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let engine = WasmEngine::new();
    let module = engine
        .compile_module(
            guest_module("exports.page_waitlist", &[(99, 0)], InvocationOutcome::Page).as_bytes(),
        )
        .unwrap();

    let error = engine
        .execute_session(&module, plan.begin_execution(), "exports.page_waitlist")
        .unwrap_err();
    assert_eq!(
        error,
        WasmModelError::InvalidHostCapabilitySlot {
            handler_id: "waitlist-page".to_string(),
            slot: 99,
        }
    );
}

#[test]
fn execution_session_tracks_peak_concurrency() {
    let manifest = page_manifest();
    let plan = InstalledExtension::install(
        manifest,
        ExtensionInstallation::new(
            "customer-app",
            vec![
                HandlerInstallation::new(
                    HandlerId::new("waitlist-page").unwrap(),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::AuthCheck,
                        HostCapabilityGrant::DataRead {
                            resource: "events.waitlist".to_string(),
                        },
                    ]),
                )
                .with_limit_override(ResourceLimits::new(
                    Duration::from_secs(2),
                    64 * 1024 * 1024,
                    4,
                    4 * 1024 * 1024,
                    2,
                    8 * 1024 * 1024,
                    2,
                )),
            ],
        )
        .unwrap(),
    )
    .unwrap()
    .prepare_invocation(
        &HandlerId::new("waitlist-page").unwrap(),
        InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-7").unwrap(),
            TraceContext::new("trace-2").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Post).unwrap(),
            ),
        ),
    )
    .unwrap();

    let mut session = plan.begin_execution();
    session.reserve_concurrency(1).unwrap();
    session.reserve_concurrency(1).unwrap();
    let err = session.reserve_concurrency(1).unwrap_err();
    assert_eq!(
        err,
        WasmModelError::ResourceLimitExceeded {
            handler_id: "waitlist-page".to_string(),
            field: "max_concurrency",
        }
    );
}

#[test]
fn extension_registry_rejects_host_api_mismatch_and_duplicate_targets() {
    let mismatched = InstalledExtension::install(
        ExtensionManifest::new(
            ExtensionId::new("admin.waitlist.future").unwrap(),
            "Future Host API",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(2, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::AdminWidget),
            vec![
                HandlerManifest::new(
                    HandlerId::new("future-widget").unwrap(),
                    "exports.future_widget",
                    ExtensionPoint::AdminWidget(
                        AdminWidgetExtensionPoint::new("admin.dashboard.summary").unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("future-widget").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
            )],
        )
        .unwrap(),
    )
    .unwrap();

    let mut registry = ExtensionRegistry::new(ContractVersion::new(1, 0, 0));
    let error = registry.install(mismatched).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::HostApiVersionMismatch {
            extension_id: "admin.waitlist.future".to_string(),
            expected: ContractVersion::new(1, 0, 0),
            actual: ContractVersion::new(2, 0, 0),
        }
    );

    let first = InstalledExtension::install(
        page_manifest(),
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page").unwrap(),
                HostGrantSet::from_grants([
                    HostCapabilityGrant::AuthCheck,
                    HostCapabilityGrant::DataRead {
                        resource: "events.waitlist".to_string(),
                    },
                    HostCapabilityGrant::RenderFragment {
                        slot: "events.waitlist.panel".to_string(),
                    },
                    HostCapabilityGrant::CacheHintWrite,
                ]),
            )],
        )
        .unwrap(),
    )
    .unwrap();
    registry.install(first).unwrap();

    let conflicting_page = InstalledExtension::install(
        ExtensionManifest::new(
            ExtensionId::new("events.waitlist.duplicate").unwrap(),
            "Duplicate Waitlist Route",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Page),
            vec![
                HandlerManifest::new(
                    HandlerId::new("waitlist-page-alt").unwrap(),
                    "exports.page_waitlist_alt",
                    ExtensionPoint::Page(
                        PageExtensionPoint::new("/events/waitlist", [HttpMethod::Get]).unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page-alt").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
            )],
        )
        .unwrap(),
    )
    .unwrap();

    let error = registry.install(conflicting_page).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::DuplicateExtensionTarget {
            point: ExtensionPointKind::Page,
            target: "GET /events/waitlist".to_string(),
            existing_handler: "events.waitlist::waitlist-page".to_string(),
            conflicting_handler: "events.waitlist.duplicate::waitlist-page-alt".to_string(),
        }
    );
}
