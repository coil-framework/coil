use super::*;

fn jobs_runtime() -> JobsRuntime {
    JobsRuntime::from_config(&davenda_config::JobsConfig {
        backend: davenda_config::JobBackend::Redis,
        retry_limit: 3,
    })
    .expect("valid jobs runtime")
}

fn sample_manifests() -> Vec<ModuleManifest> {
    vec![
        ModuleManifest::new("cms")
            .with_search_contributions(vec![ManifestSearchIndexContribution::new(
                "search.cms.pages",
                ManifestSearchDocumentKind::Page,
                ManifestSearchVisibility::Public,
                true,
                vec![ManifestSearchFieldContribution::new(
                    "title",
                    "title",
                    ManifestSearchFieldRole::Title,
                    true,
                    true,
                )],
                vec![ManifestSearchInvalidationRule::new(
                    ManifestSearchInvalidationTrigger::Published,
                    "page published",
                )],
                ManifestSearchRebuildStrategy::OnInvalidate,
            )])
            .with_bulk_operations(vec![ManifestBulkOperationDefinition::new(
                "bulk.cms.publish",
                "Bulk publish pages",
                Some("Publishes editorial pages".to_string()),
                Capability::CmsPagePublish,
                ManifestBulkOperationKind::Publish,
                ManifestBulkOperationScope::Cms,
                default_retry_policy(),
                Some(100),
                true,
            )]),
        ModuleManifest::new("commerce")
            .with_search_contributions(vec![ManifestSearchIndexContribution::new(
                "search.catalog.products",
                ManifestSearchDocumentKind::Product,
                ManifestSearchVisibility::Public,
                true,
                vec![ManifestSearchFieldContribution::new(
                    "name",
                    "name",
                    ManifestSearchFieldRole::Title,
                    true,
                    true,
                )],
                vec![ManifestSearchInvalidationRule::new(
                    ManifestSearchInvalidationTrigger::Published,
                    "product published",
                )],
                ManifestSearchRebuildStrategy::OnInvalidate,
            )])
            .with_report_definitions(vec![ManifestReportDefinition::new(
                "report.orders.summary",
                "Orders summary",
                Some("Operational summary of captured orders".to_string()),
                Capability::OrderRead,
                ManifestReportFormat::Csv,
                ManifestReportSensitivity::Restricted,
                ManifestReportDeliveryMode::InternalOnly,
                "reports/orders",
                default_retry_policy(),
            )]),
        ModuleManifest::new("events")
            .with_search_contributions(vec![ManifestSearchIndexContribution::new(
                "search.events.bookings",
                ManifestSearchDocumentKind::Booking,
                ManifestSearchVisibility::Capability(Capability::EventsBookingCheckIn),
                false,
                vec![ManifestSearchFieldContribution::new(
                    "attendee",
                    "attendee.display_name",
                    ManifestSearchFieldRole::Title,
                    true,
                    true,
                )],
                vec![ManifestSearchInvalidationRule::new(
                    ManifestSearchInvalidationTrigger::Updated,
                    "booking changed",
                )],
                ManifestSearchRebuildStrategy::ManualOnly,
            )])
            .with_bulk_operations(vec![ManifestBulkOperationDefinition::new(
                "bulk.events.check-in",
                "Bulk check in bookings",
                Some("Checks in event bookings".to_string()),
                Capability::EventsBookingCheckIn,
                ManifestBulkOperationKind::CheckIn,
                ManifestBulkOperationScope::Events,
                default_retry_policy(),
                Some(500),
                true,
            )]),
        ModuleManifest::new("media").with_search_contributions(vec![
            ManifestSearchIndexContribution::new(
                "search.media",
                ManifestSearchDocumentKind::Media,
                ManifestSearchVisibility::Public,
                true,
                vec![ManifestSearchFieldContribution::new(
                    "title",
                    "title",
                    ManifestSearchFieldRole::Title,
                    true,
                    true,
                )],
                vec![ManifestSearchInvalidationRule::new(
                    ManifestSearchInvalidationTrigger::Published,
                    "asset published",
                )],
                ManifestSearchRebuildStrategy::OnInvalidate,
            ),
        ]),
        OpsModule::new().manifest(),
    ]
}

#[test]
fn catalog_exposes_search_visibility_rules() {
    let manifests = sample_manifests();
    let catalog = OpsCatalog::from_manifests(&manifests).expect("catalog");
    catalog
        .validate()
        .expect("composed catalog should validate");

    let visible = catalog.search.visible_to(&[]);
    assert!(visible
        .iter()
        .any(|index| index.id.as_str() == "search.cms.pages"));
    assert!(visible
        .iter()
        .any(|index| index.id.as_str() == "search.media"));
    assert!(!visible
        .iter()
        .any(|index| index.id.as_str() == "search.events.bookings"));
}

#[test]
fn report_exports_are_planned_as_async_jobs_with_output_metadata() {
    let planner = OpsPlanner::new(
        jobs_runtime(),
        OpsCatalog::from_manifests(&sample_manifests()).expect("catalog"),
    )
    .expect("planner");
    let request = ReportExportRequest::new(
        ReportExportId::new("export-1").expect("valid id"),
        ReportId::new("report.orders.summary").expect("valid id"),
        "operator-1",
        JobInstant::from_unix_seconds(100),
    )
    .expect("request")
    .with_capability(Capability::OrderRead)
    .with_idempotency_key(IdempotencyKey::new("orders:summary:v1").expect("valid key"));

    let plan = planner.plan_report_export(request).expect("export plan");
    assert_eq!(plan.job.queue.as_str(), "jobs.work");
    assert_eq!(plan.definition.id.as_str(), "report.orders.summary");
    assert!(plan.planned_job.idempotency_key.is_some());
    assert_eq!(plan.output_object_key, "reports/orders/export-1.csv");
}

#[test]
fn bulk_operations_require_their_capability_and_idempotency() {
    let planner = OpsPlanner::new(
        jobs_runtime(),
        OpsCatalog::from_manifests(&sample_manifests()).expect("catalog"),
    )
    .expect("planner");
    let request = BulkOperationRequest::new(
        BulkExecutionId::new("bulk-1").expect("valid id"),
        BulkOperationId::new("bulk.events.check-in").expect("valid id"),
        "operator-2",
        JobInstant::from_unix_seconds(200),
        12,
    )
    .expect("request");

    let missing = planner.plan_bulk_operation(request.clone()).unwrap_err();
    assert!(matches!(
        missing,
        OpsModelError::MissingCapability {
            operation: "bulk operation",
            required: Capability::EventsBookingCheckIn
        }
    ));

    let plan = planner
        .plan_bulk_operation(
            request
                .with_capability(Capability::EventsBookingCheckIn)
                .with_idempotency_key(
                    IdempotencyKey::new("events:check-in:v1").expect("valid key"),
                ),
        )
        .expect("bulk plan");

    assert_eq!(plan.definition.id.as_str(), "bulk.events.check-in");
    assert!(plan.planned_job.idempotency_key.is_some());
    assert_eq!(plan.target_count, 12);
}

#[test]
fn search_and_report_definitions_are_registry_ready() {
    let module = OpsModule::new();
    let manifest = module.manifest();

    assert_eq!(manifest.name, "ops");
    assert_eq!(manifest.config_namespace.as_deref(), Some("ops"));
    assert!(manifest
        .required_capabilities
        .contains(&Capability::AdminShellAccess));
    assert!(manifest
        .optional_capabilities
        .contains(&Capability::CmsPagePublish));
    assert!(manifest
        .module_dependencies
        .iter()
        .any(|dependency| dependency.module == "admin"));
    assert_eq!(manifest.migrations.len(), 3);
    assert_eq!(manifest.route_surfaces.len(), 3);
    assert_eq!(manifest.http_surfaces.len(), 3);
    assert_eq!(manifest.jobs.len(), 3);
    assert_eq!(manifest.event_subscriptions.len(), 3);
    assert_eq!(manifest.admin_resources.len(), 3);
    assert_eq!(manifest.report_definitions.len(), 1);
    assert_eq!(manifest.bulk_operations.len(), 2);
    assert!(manifest
        .core_service_dependencies
        .contains(&CoreServiceDependency::Storage));
    assert!(manifest
        .extension_slots
        .iter()
        .any(|slot| slot.kind == ExtensionSlotKind::Job));
    assert_eq!(
        module
            .install_migration_plan()
            .expect("ops migration plan")
            .ordered_steps()
            .len(),
        3
    );

    let mut registry = ServiceRegistry::new();
    module.register(&mut registry).expect("module register");
    assert_eq!(registry.services().count(), 5);
    assert_eq!(registry.modules().count(), 0);
}
