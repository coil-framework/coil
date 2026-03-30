use super::*;

pub(super) fn build_manifest(module: &OpsModule) -> ModuleManifest {
    ModuleManifest::new(module.name().to_string())
        .with_required_capabilities(required_capabilities())
        .with_optional_capabilities(optional_capabilities())
        .with_config_namespace(module.config_namespace().to_string())
        .with_capability_contracts(capability_contracts())
        .with_module_dependencies(module_dependencies())
        .with_core_service_dependencies(core_service_dependencies())
        .with_migrations(module_migrations())
        .with_route_surfaces(route_surfaces())
        .with_jobs(jobs())
        .with_event_subscriptions(event_subscriptions())
        .with_integration_points(integration_points())
        .with_behaviors(module_behaviors())
        .with_extension_slots(extension_slots())
        .with_admin_resources(admin_resources())
        .with_report_definitions(report_definitions())
        .with_bulk_operations(bulk_operations())
        .with_http_surfaces(http_surfaces())
}

fn required_capabilities() -> Vec<Capability> {
    vec![
        Capability::AdminShellAccess,
        Capability::AdminAuditRead,
        Capability::SystemModuleManage,
    ]
}

fn optional_capabilities() -> Vec<Capability> {
    vec![
        Capability::CmsPageRead,
        Capability::CmsPageEdit,
        Capability::CmsPagePublish,
        Capability::CmsNavigationEdit,
        Capability::CatalogProductRead,
        Capability::CatalogProductEdit,
        Capability::CatalogCollectionEdit,
        Capability::OrderRead,
        Capability::OrderRefundIssue,
        Capability::MembershipSubscriptionManage,
        Capability::MembershipTierEdit,
        Capability::EventsEventPublish,
        Capability::EventsSlotManage,
        Capability::EventsBookingCreate,
        Capability::EventsBookingCheckIn,
        Capability::AssetRead,
        Capability::AssetReadPublic,
        Capability::AssetPublish,
        Capability::AssetReplace,
        Capability::AssetManageStorage,
    ]
}

fn capability_contracts() -> Vec<CapabilityContract> {
    vec![
        CapabilityContract::required(Capability::AdminShellAccess, ["admin_module"]),
        CapabilityContract::required(Capability::AdminAuditRead, ["audit_entry"]),
        CapabilityContract::required(Capability::SystemModuleManage, ["admin_module"]),
        CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
        CapabilityContract::optional(Capability::CmsPageEdit, ["page"]),
        CapabilityContract::optional(Capability::CmsPagePublish, ["page"]),
        CapabilityContract::optional(Capability::CmsNavigationEdit, ["navigation"]),
        CapabilityContract::optional(Capability::CatalogProductRead, ["product"]),
        CapabilityContract::optional(Capability::CatalogProductEdit, ["product"]),
        CapabilityContract::optional(Capability::CatalogCollectionEdit, ["collection"]),
        CapabilityContract::optional(Capability::OrderRead, ["order"]),
        CapabilityContract::optional(Capability::OrderRefundIssue, ["order"]),
        CapabilityContract::optional(Capability::MembershipSubscriptionManage, ["subscription"]),
        CapabilityContract::optional(Capability::MembershipTierEdit, ["membership_tier"]),
        CapabilityContract::optional(Capability::EventsEventPublish, ["event"]),
        CapabilityContract::optional(Capability::EventsSlotManage, ["event_slot"]),
        CapabilityContract::optional(Capability::EventsBookingCreate, ["booking"]),
        CapabilityContract::optional(Capability::EventsBookingCheckIn, ["booking"]),
        CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
        CapabilityContract::optional(Capability::AssetReadPublic, ["asset", "media"]),
        CapabilityContract::optional(Capability::AssetPublish, ["asset", "media"]),
        CapabilityContract::optional(Capability::AssetReplace, ["asset", "media"]),
        CapabilityContract::optional(
            Capability::AssetManageStorage,
            ["asset", "asset_folder", "media_library"],
        ),
    ]
}

fn module_dependencies() -> Vec<ModuleDependency> {
    vec![
        ModuleDependency::required(
            "admin",
            "Operational search, reporting, and bulk actions surface through the shared admin shell",
        ),
        ModuleDependency::optional(
            "cms",
            "Search and bulk publishing can index and mutate CMS content when that module is installed",
        ),
        ModuleDependency::optional(
            "commerce",
            "Reporting and bulk operations can project order and catalog activity when commerce is installed",
        ),
        ModuleDependency::optional(
            "memberships",
            "Operational reports can include subscription and entitlement state when memberships is installed",
        ),
        ModuleDependency::optional(
            "events",
            "Search and bulk workflows can operate on bookings and check-in state when events is installed",
        ),
        ModuleDependency::optional(
            "media",
            "Search and reporting can include managed assets and storage-policy inventory when media is installed",
        ),
    ]
}

fn core_service_dependencies() -> Vec<CoreServiceDependency> {
    vec![
        CoreServiceDependency::Auth,
        CoreServiceDependency::Data,
        CoreServiceDependency::Jobs,
        CoreServiceDependency::Storage,
        CoreServiceDependency::Cache,
        CoreServiceDependency::Observability,
    ]
}

fn module_migrations() -> Vec<MigrationContract> {
    vec![
        MigrationContract::new(
            "ops.search",
            10,
            "Creates search projection and rebuild-cursor tables for first-party indexing",
        ),
        MigrationContract::new(
            "ops.reports",
            20,
            "Creates report definition, export job, and output artifact metadata tables",
        ),
        MigrationContract::new(
            "ops.bulk",
            30,
            "Creates bulk-operation intent, idempotency, and audit coordination tables",
        ),
    ]
}

fn route_surfaces() -> Vec<RouteSurface> {
    vec![
        RouteSurface::new("ops.search", RouteSurfaceKind::AdminPage, "/admin/search")
            .gated_by(Capability::AdminShellAccess),
        RouteSurface::new("ops.reports", RouteSurfaceKind::AdminPage, "/admin/reports")
            .gated_by(Capability::AdminAuditRead),
        RouteSurface::new(
            "ops.report.export",
            RouteSurfaceKind::AdminAction,
            "/admin/reports/export",
        )
        .gated_by(Capability::AdminAuditRead),
        RouteSurface::new(
            "ops.recovery.execute",
            RouteSurfaceKind::AdminAction,
            "/admin/recovery",
        )
        .gated_by(Capability::SystemModuleManage),
        RouteSurface::new(
            "ops.bulk.execute",
            RouteSurfaceKind::AdminAction,
            "/admin/bulk",
        )
        .gated_by(Capability::SystemModuleManage),
    ]
}

fn jobs() -> Vec<JobContract> {
    vec![
        JobContract::new(
            "ops.search.rebuild",
            JobTriggerKind::DomainEvent,
            true,
            "Rebuilds or repairs first-party search projections as domain records change",
        ),
        JobContract::new(
            "ops.report.export",
            JobTriggerKind::Operator,
            true,
            "Runs asynchronous report exports and persists their output artifacts safely",
        ),
        JobContract::new(
            "ops.bulk.execute",
            JobTriggerKind::Operator,
            true,
            "Executes audited bulk workflows behind idempotent job envelopes",
        ),
        JobContract::new(
            "ops.recovery.rehydrate",
            JobTriggerKind::Operator,
            true,
            "Rebuilds derived state and recovery steps after source-of-truth restore completes",
        ),
    ]
}

fn event_subscriptions() -> Vec<EventSubscription> {
    vec![
        EventSubscription::new(
            "cms.page.published",
            Some("ops.search.rebuild"),
            "Refreshes search projections after editorial publication changes",
        ),
        EventSubscription::new(
            "commerce.order.paid",
            Some("ops.report.export"),
            "Enables scheduled or on-demand reporting to capture completed transactional activity",
        ),
        EventSubscription::new(
            "events.booking.checked-in",
            Some("ops.bulk.execute"),
            "Keeps operational bulk and reporting views consistent with live attendance changes",
        ),
    ]
}

fn integration_points() -> Vec<IntegrationPoint> {
    vec![
        IntegrationPoint::new(
            IntegrationKind::SearchIndex,
            "ops.search",
            "Collects explicit indexing contributions from official modules and customer app extensions",
        ),
        IntegrationPoint::new(
            IntegrationKind::AdminWorkflow,
            "ops.bulk",
            "Adds report export and bulk workflow surfaces into the shared admin shell",
        ),
        IntegrationPoint::new(
            IntegrationKind::StoragePolicy,
            "ops.report-output",
            "Routes generated report artifacts through the shared storage-policy and delivery model",
        ),
        IntegrationPoint::new(
            IntegrationKind::StoragePolicy,
            "ops.recovery",
            "Coordinates recovery ordering across Postgres, managed object storage, and local-only sensitive exceptions",
        ),
    ]
}

fn module_behaviors() -> Vec<ModuleBehavior> {
    vec![
        ModuleBehavior::AsyncJobs,
        ModuleBehavior::AuditedBulkActions,
    ]
}

fn extension_slots() -> Vec<ExtensionSlotDescriptor> {
    vec![
        ExtensionSlotDescriptor::new(
            ExtensionSlotKind::AdminWidget,
            "ops.report.dashboard",
            "Allows bounded customer widgets to contribute operator metrics and report affordances",
        ),
        ExtensionSlotDescriptor::new(
            ExtensionSlotKind::Job,
            "ops.search.adapter",
            "Allows search backends to participate through explicit background job contracts",
        ),
    ]
}

fn admin_resources() -> Vec<AdminResourceContribution> {
    vec![
        AdminResourceContribution::new(
            "ops.search",
            "/admin/search",
            "Search",
            "Search",
            AdminNavigationSection::System,
            AdminContributionKind::ResourceIndex,
            Capability::AdminShellAccess,
        ),
        AdminResourceContribution::new(
            "ops.reports",
            "/admin/reports",
            "Reports",
            "Reports",
            AdminNavigationSection::System,
            AdminContributionKind::ResourceIndex,
            Capability::AdminAuditRead,
        ),
        AdminResourceContribution::new(
            "ops.bulk",
            "/admin/bulk",
            "Bulk operations",
            "Bulk",
            AdminNavigationSection::System,
            AdminContributionKind::Workflow,
            Capability::SystemModuleManage,
        ),
        AdminResourceContribution::new(
            "ops.recovery",
            "/admin/recovery",
            "Recovery",
            "Recovery",
            AdminNavigationSection::System,
            AdminContributionKind::Workflow,
            Capability::SystemModuleManage,
        ),
    ]
}

fn report_definitions() -> Vec<ManifestReportDefinition> {
    vec![
        ManifestReportDefinition::new(
            "report.ops.search-health",
            "Search health",
            Some(
                "Operational visibility into index freshness, drift, and rebuild lag"
                    .to_string(),
            ),
            Capability::AdminAuditRead,
            ManifestReportFormat::Json,
            ManifestReportSensitivity::Internal,
            ManifestReportDeliveryMode::SignedUrl,
            "reports/ops/search",
            default_retry_policy(),
        ),
        ManifestReportDefinition::new(
            "report.ops.backup-readiness",
            "Backup readiness",
            Some(
                "Summarizes whether source-of-truth data classes follow the platform recovery model"
                    .to_string(),
            ),
            Capability::AdminAuditRead,
            ManifestReportFormat::Json,
            ManifestReportSensitivity::Internal,
            ManifestReportDeliveryMode::SignedUrl,
            "reports/ops/backup",
            default_retry_policy(),
        ),
    ]
}

fn bulk_operations() -> Vec<ManifestBulkOperationDefinition> {
    vec![
        ManifestBulkOperationDefinition::new(
            "bulk.search.reindex",
            "Reindex search",
            Some("Queues a coordinated rebuild across declared search indexes".to_string()),
            Capability::SystemModuleManage,
            ManifestBulkOperationKind::Reindex,
            ManifestBulkOperationScope::Search,
            default_retry_policy(),
            Some(100),
            true,
        ),
        ManifestBulkOperationDefinition::new(
            "bulk.reports.export",
            "Bulk export reports",
            Some("Queues exports for multiple reports without request-time blocking".to_string()),
            Capability::AdminAuditRead,
            ManifestBulkOperationKind::Export,
            ManifestBulkOperationScope::System,
            default_retry_policy(),
            Some(50),
            true,
        ),
    ]
}

fn http_surfaces() -> Vec<HttpSurfaceContribution> {
    vec![
        HttpSurfaceContribution::page(
            "ops.search",
            HttpSurfaceArea::Admin,
            "/admin/search",
            "ops/search",
        )
        .gated_by(Capability::AdminShellAccess),
        HttpSurfaceContribution::page(
            "ops.reports",
            HttpSurfaceArea::Admin,
            "/admin/reports",
            "ops/reports",
        )
        .gated_by(Capability::AdminAuditRead),
        HttpSurfaceContribution::json(
            "ops.report.export",
            HttpSurfaceMethod::Post,
            HttpSurfaceArea::Admin,
            "/admin/reports/export",
            202,
            std::collections::BTreeMap::from([("status".to_string(), "queued".to_string())]),
        )
        .gated_by(Capability::AdminAuditRead),
        HttpSurfaceContribution::page(
            "ops.recovery",
            HttpSurfaceArea::Admin,
            "/admin/recovery",
            "ops/recovery",
        )
        .gated_by(Capability::SystemModuleManage),
        HttpSurfaceContribution::page(
            "ops.bulk",
            HttpSurfaceArea::Admin,
            "/admin/bulk",
            "ops/bulk",
        )
        .gated_by(Capability::SystemModuleManage),
        HttpSurfaceContribution::json(
            "ops.recovery.execute",
            HttpSurfaceMethod::Post,
            HttpSurfaceArea::Admin,
            "/admin/recovery",
            202,
            std::collections::BTreeMap::from([("status".to_string(), "queued".to_string())]),
        )
        .gated_by(Capability::SystemModuleManage),
        HttpSurfaceContribution::json(
            "ops.bulk.execute",
            HttpSurfaceMethod::Post,
            HttpSurfaceArea::Admin,
            "/admin/bulk",
            202,
            std::collections::BTreeMap::from([("status".to_string(), "queued".to_string())]),
        )
        .gated_by(Capability::SystemModuleManage),
    ]
}
