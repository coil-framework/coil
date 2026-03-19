use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModule {
    name: String,
    config_namespace: String,
}

impl OpsModule {
    pub fn new() -> Self {
        Self {
            name: "ops".to_string(),
            config_namespace: "ops".to_string(),
        }
    }

    pub fn planner(
        &self,
        runtime: JobsRuntime,
        manifests: &[ModuleManifest],
    ) -> Result<OpsPlanner, OpsModelError> {
        OpsPlanner::new(runtime, OpsCatalog::from_manifests(manifests)?)
    }
}

impl Default for OpsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for OpsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::AdminAuditRead,
                Capability::SystemModuleManage,
            ])
            .with_optional_capabilities(vec![
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
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
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
            ])
            .with_module_dependencies(vec![
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
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Storage,
                CoreServiceDependency::Cache,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
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
            ])
            .with_route_surfaces(vec![
                RouteSurface::new("ops.search", RouteSurfaceKind::AdminPage, "/admin/search")
                    .gated_by(Capability::AdminShellAccess),
                RouteSurface::new("ops.reports", RouteSurfaceKind::AdminPage, "/admin/reports")
                    .gated_by(Capability::AdminAuditRead),
                RouteSurface::new("ops.bulk", RouteSurfaceKind::AdminAction, "/admin/bulk")
                    .gated_by(Capability::SystemModuleManage),
            ])
            .with_jobs(vec![
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
            ])
            .with_event_subscriptions(vec![
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
            ])
            .with_integration_points(vec![
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
            ])
            .with_behaviors(vec![
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AuditedBulkActions,
            ])
            .with_extension_slots(vec![
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
            ])
            .with_admin_resources(vec![
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
            ])
            .with_report_definitions(vec![ManifestReportDefinition::new(
                "report.ops.search-health",
                "Search health",
                Some("Operational visibility into index freshness, drift, and rebuild lag".to_string()),
                Capability::AdminAuditRead,
                ManifestReportFormat::Json,
                ManifestReportSensitivity::Internal,
                ManifestReportDeliveryMode::SignedUrl,
                "reports/ops/search",
                default_retry_policy(),
            )])
            .with_bulk_operations(vec![
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
            ])
            .with_http_surfaces(vec![
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
                    "ops.bulk",
                    HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/bulk",
                    202,
                    std::collections::BTreeMap::from([(
                        "status".to_string(),
                        "queued".to_string(),
                    )]),
                )
                .gated_by(Capability::SystemModuleManage),
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.ops.search",
            "Declarative search indexing contributions, visibility rules, and rebuild metadata",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.reports",
            "Asynchronous report definitions, export planning, and delivery policies",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.bulk",
            "Capability-gated bulk operations with audit-ready, idempotent job planning",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.jobs",
            "Jobs-backed execution planning for reports and bulk workflows",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.ops.audit",
            "Operator visibility into search, reporting, and bulk action plans",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("ops_search").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create search projection and rebuild cursor storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS ops_search_projection (id TEXT PRIMARY KEY, document_type TEXT NOT NULL, visibility TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("ops migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("ops_reports").expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create report definition and export artifact storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS ops_reports (id TEXT PRIMARY KEY, format TEXT NOT NULL, output_path TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("ops migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("ops_bulk").expect("constant migration id is valid"),
                owner,
                30,
                "Create bulk workflow intent and idempotency storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS ops_bulk_operations (id TEXT PRIMARY KEY, action TEXT NOT NULL, idempotency_key TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("ops migration ids are unique");
        Some(plan)
    }
}
