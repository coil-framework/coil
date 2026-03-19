use super::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
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

fn wat_bytes_literal(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("\\{:02x}", byte))
        .collect::<Vec<_>>()
        .join("")
}

fn guest_module_with_output(
    export: &str,
    host_calls: &[(i32, i64)],
    outcome: InvocationOutcome,
    output_bytes: &[u8],
) -> String {
    let mut body = String::new();
    for (slot, metric) in host_calls {
        body.push_str(&format!(
            "    i32.const {slot}\n    i64.const {metric}\n    call $host_call\n    drop\n"
        ));
    }

    let packed_output = ((output_bytes.len() as u64) << 32) | 0u64;
    format!(
        "(module
            (import \"davenda\" \"host_call\" (func $host_call (param i32 i64) (result i32)))
            (memory (export \"memory\") 1)
            (data (i32.const 0) \"{}\")
            (func (export \"{export}\") (result i32)
{body}                i32.const {}
            )
            (func (export \"{}\") (result i64)
                i64.const {}
            )
        )",
        wat_bytes_literal(output_bytes),
        outcome.engine_code(),
        TypedExecutionOutput::ABI_EXPORT,
        packed_output
    )
}

fn guest_module_with_typed_output_len(
    export: &str,
    host_calls: &[(i32, i64)],
    outcome: InvocationOutcome,
    output_bytes: &[u8],
    reported_len: usize,
) -> String {
    let mut body = String::new();
    for (slot, metric) in host_calls {
        body.push_str(&format!(
            "    i32.const {slot}\n    i64.const {metric}\n    call $host_call\n    drop\n"
        ));
    }

    let packed_output = ((reported_len as u64) << 32) | 0u64;
    format!(
        "(module
            (import \"davenda\" \"host_call\" (func $host_call (param i32 i64) (result i32)))
            (memory (export \"memory\") 1)
            (data (i32.const 0) \"{}\")
            (func (export \"{export}\") (result i32)
{body}                i32.const {}
            )
            (func (export \"{}\") (result i64)
                i64.const {}
            )
        )",
        wat_bytes_literal(output_bytes),
        outcome.engine_code(),
        TypedExecutionOutput::ABI_EXPORT,
        packed_output
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

fn typed_page_output() -> TypedExecutionOutput {
    let metadata = TypedMetadata::new()
        .with_title("Spring Tasting")
        .unwrap()
        .with_description("Seasonal event page")
        .unwrap()
        .with_canonical_url("https://www.example.com/en-GB/events/spring-tasting")
        .unwrap()
        .insert_alternate_url(
            "fr-FR",
            "https://www.example.com/fr-FR/events/spring-tasting",
        )
        .unwrap()
        .with_robot_directive(RobotsDirective::Index)
        .with_robot_directive(RobotsDirective::Follow)
        .push_json_ld(
            JsonLdNode::new("WebPage")
                .unwrap()
                .set_string("name", "Spring Tasting")
                .unwrap()
                .set_string("url", "https://www.example.com/en-GB/events/spring-tasting")
                .unwrap(),
        );

    let cache_hint = TypedCacheHint::new(
        CacheVisibility::Public,
        300,
        Some(30),
        true,
        false,
        false,
        ["events", "spring-tasting"],
    )
    .unwrap();

    TypedExecutionOutput::page(
        200,
        "<section><h1>Spring Tasting</h1></section>",
        metadata,
        Some(cache_hint),
    )
    .unwrap()
}

fn typed_api_output() -> TypedExecutionOutput {
    TypedExecutionOutput::api(
        200,
        BTreeMap::from([("extension".to_string(), "ok".to_string())]),
        TypedMetadata::new()
            .with_title("API Response")
            .unwrap()
            .with_description("typed API output")
            .unwrap(),
        None,
    )
    .unwrap()
}

fn typed_admin_widget_output() -> TypedExecutionOutput {
    TypedExecutionOutput::admin_widget(
        200,
        "<section><h2>Admin</h2></section>",
        TypedMetadata::new()
            .with_description("admin widget output")
            .unwrap(),
        None,
    )
    .unwrap()
}

fn typed_render_hook_output() -> TypedExecutionOutput {
    TypedExecutionOutput::render_hook(
        200,
        "<aside>Render hook</aside>",
        TypedMetadata::new()
            .with_description("render hook output")
            .unwrap(),
        None,
    )
    .unwrap()
}

fn assert_typed_output_roundtrips(point: ExtensionPointKind, output: TypedExecutionOutput) {
    let encoded = output.encode().unwrap();
    let decoded = match point {
        ExtensionPointKind::Page => TypedExecutionOutput::decode_page(&encoded).unwrap(),
        ExtensionPointKind::Api => TypedExecutionOutput::decode_api(&encoded).unwrap(),
        ExtensionPointKind::AdminWidget => {
            TypedExecutionOutput::decode_admin_widget(&encoded).unwrap()
        }
        ExtensionPointKind::RenderHook => {
            TypedExecutionOutput::decode_render_hook(&encoded).unwrap()
        }
        other => panic!("unexpected typed surface in test: {other:?}"),
    };
    assert_eq!(decoded, output);
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

    let mut session = plan.begin_synthetic_execution();
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

    assert_eq!(session.host_service_executions().len(), 3);
    assert!(matches!(
        &session.host_service_executions()[0].result,
        HostServiceResult::Auth(AuthServiceExecution {
            allowed: true,
            checks_seen: 1,
            details: AuthServiceDetails::Check {
                capability,
                object,
                decision: true,
            },
            ..
        }) if capability == "system.config.read" && object == "tenant:customer-app"
    ));
    assert!(matches!(
        &session.host_service_executions()[1].result,
        HostServiceResult::Network(NetworkExecution {
            integration,
            response_bytes: 512,
            ..
        }) if integration == "crm"
    ));
    assert!(matches!(
        &session.host_service_executions()[2].result,
        HostServiceResult::Storage(StorageServiceExecution {
            total_bytes: 1_024,
            ..
        })
    ));

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
fn execution_session_defaults_to_a_denied_host_executor() {
    let manifest = page_manifest();
    let plan = InstalledExtension::install(
        manifest,
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
            PrincipalRef::user("user-42").unwrap(),
            TraceContext::new("trace-default-deny").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let error = plan
        .begin_execution()
        .record_host_call(HostCall::AuthCheck)
        .unwrap_err();

    assert!(matches!(
        error,
        WasmModelError::HostServiceUnavailable {
            domain: HostServiceDomain::Auth,
            ..
        }
    ));
}

#[test]
fn execution_session_wraps_data_access_in_a_module_owned_contract() {
    let manifest = page_manifest();
    let plan = InstalledExtension::install(
        manifest,
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::DataRead {
                    resource: "events.waitlist".to_string(),
                }]),
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
            TraceContext::new("trace-data-contract").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let mut session = plan.begin_synthetic_execution();
    let execution = session
        .execute_host_call(HostCall::DataRead {
            resource: "events.waitlist".to_string(),
        })
        .unwrap();

    assert!(matches!(
        execution.result,
        HostServiceResult::Data(DataServiceExecution {
            request: DataServiceRequest::Read { contract },
            summary,
            ..
        }) if contract.owner_extension_id == "events.waitlist"
            && contract.owner_handler_id == "waitlist-page"
            && contract.repository.id == "events.waitlist"
            && matches!(contract.repository.kind, ModuleDataRepositoryKind::Owned)
            && summary.contains("repository=events.waitlist")
            && summary.contains("repository_kind=owned")
            && summary.contains("access=read")
    ));
}

#[test]
fn module_data_contract_carries_a_typed_repository_binding() {
    let repository = ModuleDataRepository::shared("cms.pages").unwrap();
    let contract =
        ModuleDataContract::with_repository("cms.pages", "page-reader", repository.clone())
            .unwrap();

    assert_eq!(contract.repository_id(), "cms.pages");
    assert_eq!(contract.resource, "cms.pages");
    assert_eq!(contract.repository, repository);
    assert!(
        contract
            .summary("read", 7)
            .contains("repository_kind=shared")
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
        .begin_synthetic_execution()
        .finish(Duration::from_secs(1), InvocationOutcome::ApiJson, None)
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
        .begin_synthetic_execution()
        .finish(
            Duration::from_secs(31),
            InvocationOutcome::JobCompleted,
            None,
        )
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
        .execute_session(
            &module,
            plan.begin_synthetic_execution(),
            "exports.page_waitlist",
        )
        .unwrap();
    assert_eq!(receipt.outcome, InvocationOutcome::Page);
    assert_eq!(receipt.point, ExtensionPointKind::Page);
}

#[test]
fn wasm_engine_collects_typed_return_payloads_from_guest_memory() {
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
            PrincipalRef::user("user-99").unwrap(),
            TraceContext::new("trace-output").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let output = typed_page_output();
    let encoded = output.encode().unwrap();
    let engine = WasmEngine::new();
    let module = engine
        .compile_module(
            guest_module_with_output(
                "exports.page_waitlist",
                &[(0, 0)],
                InvocationOutcome::Page,
                &encoded,
            )
            .as_bytes(),
        )
        .unwrap();

    let receipt = engine
        .execute_session(
            &module,
            plan.begin_synthetic_execution(),
            "exports.page_waitlist",
        )
        .unwrap();

    assert_eq!(receipt.outcome, InvocationOutcome::Page);
    assert_eq!(receipt.point, ExtensionPointKind::Page);
    assert_eq!(receipt.typed_output, Some(output));
}

#[test]
fn typed_execution_output_roundtrips_for_all_supported_surfaces() {
    assert_typed_output_roundtrips(ExtensionPointKind::Page, typed_page_output());
    assert_typed_output_roundtrips(ExtensionPointKind::Api, typed_api_output());
    assert_typed_output_roundtrips(ExtensionPointKind::AdminWidget, typed_admin_widget_output());
    assert_typed_output_roundtrips(ExtensionPointKind::RenderHook, typed_render_hook_output());
}

#[test]
fn typed_execution_output_rejects_invalid_http_statuses() {
    let error = TypedExecutionOutput::page(
        99,
        "<section><h1>Invalid</h1></section>",
        TypedMetadata::new(),
        None,
    )
    .unwrap_err();
    assert_eq!(error, WasmModelError::InvalidTypedStatus { status: 99 });
}

#[test]
fn typed_execution_output_rejects_surface_mismatches_on_decode() {
    let api_encoded = typed_api_output().encode().unwrap();
    let error = TypedExecutionOutput::decode_page(&api_encoded).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::TypedReturnPointMismatch {
            expected: ExtensionPointKind::Page,
            actual: ExtensionPointKind::Api,
        }
    );

    let mut body_mismatch = api_encoded;
    body_mismatch[6] = 0;
    let error = TypedExecutionOutput::decode_page(&body_mismatch).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::TypedReturnBodyMismatch {
            point: ExtensionPointKind::Page,
            body: "json_object".to_string(),
        }
    );
}

#[test]
fn wasm_engine_rejects_typed_output_payloads_outside_guest_memory() {
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
            TraceContext::new("trace-output-overrun").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let output = typed_page_output();
    let encoded = output.encode().unwrap();
    let engine = WasmEngine::new();
    let module = engine
        .compile_module(
            guest_module_with_typed_output_len(
                "exports.page_waitlist",
                &[(0, 0)],
                InvocationOutcome::Page,
                &encoded,
                encoded.len() + 65_536,
            )
            .as_bytes(),
        )
        .unwrap();

    let error = engine
        .execute_session(
            &module,
            plan.begin_synthetic_execution(),
            "exports.page_waitlist",
        )
        .unwrap_err();
    assert!(matches!(
        error,
        WasmModelError::InvalidTypedReturn { reason }
            if reason.contains("exceeds guest memory size")
    ));
}

#[test]
fn typed_execution_output_rejects_invalid_metadata_and_cache_hints() {
    let invalid_metadata = TypedExecutionOutput {
        surface: ExtensionPointKind::Page,
        status: 200,
        body: TypedResponseBody::HtmlDocument("<section>ok</section>".to_string()),
        metadata: TypedMetadata {
            title: Some("   ".to_string()),
            description: Some("Valid description".to_string()),
            canonical_url: None,
            alternate_urls: BTreeMap::new(),
            robots: BTreeSet::new(),
            json_ld: Vec::new(),
        },
        cache_hint: None,
    };
    let error = invalid_metadata.encode().unwrap_err();
    assert!(matches!(error, WasmModelError::InvalidTypedReturn { .. }));

    let invalid_cache_hint = TypedExecutionOutput {
        surface: ExtensionPointKind::Page,
        status: 200,
        body: TypedResponseBody::HtmlDocument("<section>ok</section>".to_string()),
        metadata: TypedMetadata::new(),
        cache_hint: Some(TypedCacheHint {
            visibility: CacheVisibility::Public,
            max_age_seconds: 60,
            stale_while_revalidate_seconds: None,
            vary_by_locale: false,
            vary_by_user: true,
            vary_by_session: false,
            tags: BTreeSet::new(),
        }),
    };
    let error = invalid_cache_hint.encode().unwrap_err();
    assert!(matches!(error, WasmModelError::InvalidTypedReturn { .. }));
}

#[test]
fn typed_return_payload_rejects_body_point_mismatches() {
    let output =
        TypedExecutionOutput::admin_widget(200, "<div>Admin</div>", TypedMetadata::new(), None)
            .unwrap();

    let error = TypedExecutionOutput::decode_page(&output.encode().unwrap()).unwrap_err();
    assert_eq!(
        error,
        WasmModelError::TypedReturnPointMismatch {
            expected: ExtensionPointKind::Page,
            actual: ExtensionPointKind::AdminWidget,
        }
    );
}

#[test]
fn wasm_engine_rejects_typed_output_point_mismatches() {
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
            TraceContext::new("trace-point-mismatch").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
            ),
        ),
    )
    .unwrap();

    let output = typed_admin_widget_output();
    let encoded = output.encode().unwrap();
    let engine = WasmEngine::new();
    let module = engine
        .compile_module(
            guest_module_with_output(
                "exports.page_waitlist",
                &[(0, 0)],
                InvocationOutcome::Page,
                &encoded,
            )
            .as_bytes(),
        )
        .unwrap();

    let error = engine
        .execute_session(
            &module,
            plan.begin_synthetic_execution(),
            "exports.page_waitlist",
        )
        .unwrap_err();
    assert_eq!(
        error,
        WasmModelError::TypedReturnPointMismatch {
            expected: ExtensionPointKind::Page,
            actual: ExtensionPointKind::AdminWidget,
        }
    );
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
        .execute_session(
            &module,
            plan.begin_synthetic_execution(),
            "exports.page_waitlist",
        )
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

    let mut session = plan.begin_synthetic_execution();
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
