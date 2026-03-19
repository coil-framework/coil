use super::*;

pub struct MembershipsModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceContribution>,
}

impl MembershipsModule {
    pub fn new() -> Self {
        Self {
            name: "memberships".to_string(),
            config_namespace: "memberships".to_string(),
            admin_resources: vec![
                AdminResourceContribution::new(
                    "memberships.tiers",
                    "/admin/memberships/tiers",
                    "Membership tiers",
                    "Tiers",
                    AdminNavigationSection::Memberships,
                    AdminContributionKind::ResourceIndex,
                    Capability::MembershipTierEdit,
                ),
                AdminResourceContribution::new(
                    "memberships.subscriptions",
                    "/admin/memberships/subscriptions",
                    "Subscriptions",
                    "Subscriptions",
                    AdminNavigationSection::Memberships,
                    AdminContributionKind::ResourceIndex,
                    Capability::MembershipSubscriptionManage,
                ),
            ],
        }
    }

    pub fn admin_resources(&self) -> &[AdminResourceContribution] {
        &self.admin_resources
    }
}

impl Default for MembershipsModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for MembershipsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::MembershipSubscriptionManage,
                Capability::MembershipTierEdit,
            ])
            .with_optional_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::OrderRead,
                Capability::I18nTranslationEdit,
                Capability::AssetRead,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(
                    Capability::MembershipSubscriptionManage,
                    ["subscription"],
                ),
                CapabilityContract::required(
                    Capability::MembershipTierEdit,
                    ["membership_tier"],
                ),
                CapabilityContract::optional(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::optional(Capability::OrderRead, ["order"]),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["membership_tier"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::required(
                    "commerce",
                    "Membership subscriptions are provisioned from order outcomes and billing lifecycles",
                ),
                ModuleDependency::optional(
                    "admin",
                    "Memberships contributes operator resources into the shared admin shell when installed",
                ),
                ModuleDependency::optional(
                    "events",
                    "Membership tiers can influence event eligibility and member-only booking workflows",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Observability,
                CoreServiceDependency::I18n,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "memberships.tiers",
                    10,
                    "Creates membership tier, benefit, and merchandising policy tables",
                ),
                MigrationContract::new(
                    "memberships.subscriptions",
                    20,
                    "Creates subscription lifecycle state, term, and grace-period tables",
                ),
                MigrationContract::new(
                    "memberships.entitlements",
                    30,
                    "Creates entitlement grants and revocation audit rows linked to active subscriptions",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new(
                    "memberships.account",
                    RouteSurfaceKind::FrontendPage,
                    "/account/memberships",
                )
                .gated_by(Capability::MembershipSubscriptionManage),
                RouteSurface::new(
                    "memberships.tiers",
                    RouteSurfaceKind::AdminPage,
                    "/admin/memberships/tiers",
                )
                .gated_by(Capability::MembershipTierEdit),
                RouteSurface::new(
                    "memberships.subscriptions",
                    RouteSurfaceKind::AdminPage,
                    "/admin/memberships/subscriptions",
                )
                .gated_by(Capability::MembershipSubscriptionManage),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "memberships.renewals",
                    JobTriggerKind::Scheduled,
                    true,
                    "Processes scheduled renewals, grace-period transitions, and retry windows",
                ),
                JobContract::new(
                    "memberships.entitlements.sync",
                    JobTriggerKind::DomainEvent,
                    true,
                    "Reconciles auth-backed entitlements after subscription lifecycle changes",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "commerce.order.paid",
                    Some("memberships.entitlements.sync"),
                    "Creates or extends subscription access after qualifying membership purchases complete",
                ),
                EventSubscription::new(
                    "membership.subscription.renewal-due",
                    Some("memberships.renewals"),
                    "Schedules renewal and grace-period maintenance work for active subscriptions",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::AdminNavigation,
                    "admin.memberships",
                    "Adds tier and subscription management resources to the shared operator shell",
                ),
                IntegrationPoint::new(
                    IntegrationKind::CommerceBridge,
                    "commerce.orders",
                    "Projects order outcomes into recurring membership state and entitlement grants",
                ),
                IntegrationPoint::new(
                    IntegrationKind::FrontendRendering,
                    "account.memberships",
                    "Provides the member account experience and entitlement visibility surface",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::AccessibleAdminUi,
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AuditedBulkActions,
            ])
            .with_extension_slots(vec![ExtensionSlotDescriptor::new(
                ExtensionSlotKind::AdminWidget,
                "memberships.subscription.summary",
                "Allows customer app widgets to augment subscription detail views with bounded insights",
            )])
            .with_admin_resources(self.admin_resources.clone())
            .with_search_contributions(vec![SearchIndexContribution::new(
                "search.memberships",
                SearchDocumentKind::MembershipSubscription,
                SearchVisibility::Capability(Capability::MembershipSubscriptionManage),
                false,
                vec![
                    SearchFieldContribution::new(
                        "tier",
                        "tier_name",
                        SearchFieldRole::Title,
                        true,
                        true,
                    ),
                    SearchFieldContribution::new(
                        "status",
                        "status",
                        SearchFieldRole::Facet,
                        true,
                        true,
                    ),
                ],
                vec![
                    SearchInvalidationRule::new(
                        SearchInvalidationTrigger::Updated,
                        "subscription changed",
                    ),
                    SearchInvalidationRule::new(
                        SearchInvalidationTrigger::ManualRebuild,
                        "membership audit rebuild",
                    ),
                ],
                SearchRebuildStrategy::ManualOnly,
            )])
            .with_report_definitions(vec![ReportDefinition::new(
                "report.memberships.summary",
                "Subscription summary",
                Some("Lifecycle summary for memberships and renewals".to_string()),
                Capability::MembershipSubscriptionManage,
                ReportFormat::Csv,
                ReportSensitivity::Internal,
                ReportDeliveryMode::SignedUrl,
                "reports/memberships",
                default_retry_policy(),
            )])
            .with_http_surfaces(vec![
                HttpSurfaceContribution::page(
                    "memberships.account",
                    HttpSurfaceArea::Account,
                    "/account/memberships",
                    "memberships/account",
                )
                .gated_by(Capability::MembershipSubscriptionManage),
                HttpSurfaceContribution::page(
                    "memberships.tiers",
                    HttpSurfaceArea::Admin,
                    "/admin/memberships/tiers",
                    "memberships/tiers",
                )
                .gated_by(Capability::MembershipTierEdit),
                HttpSurfaceContribution::page(
                    "memberships.subscriptions",
                    HttpSurfaceArea::Admin,
                    "/admin/memberships/subscriptions",
                    "memberships/subscriptions",
                )
                .gated_by(Capability::MembershipSubscriptionManage),
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.tiers",
            "Membership tiers, benefits, and plan configuration",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.subscriptions",
            "Subscription lifecycle, grace periods, pause and cancellation handling",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.entitlements",
            "Entitlement grants and revocation aligned with auth-backed member access",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.renewals",
            "Renewal scheduling, retry orchestration, and subscription follow-up work",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.commerce_bridge",
            "Commerce order outcomes translated into membership subscription state",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.memberships.admin",
            "Membership operator resources for tiers, subscriptions, and entitlement review",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("membership_tiers").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create membership tier and benefit storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS membership_tiers (id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("membership migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("membership_subscriptions")
                    .expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create subscription lifecycle and renewal state storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS membership_subscriptions (id TEXT PRIMARY KEY, tier_id TEXT NOT NULL, status TEXT NOT NULL, renews_at BIGINT)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("membership migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("membership_entitlements")
                    .expect("constant migration id is valid"),
                owner,
                30,
                "Create entitlement grant and revocation audit storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS membership_entitlements (id TEXT PRIMARY KEY, subscription_id TEXT NOT NULL, entitlement_key TEXT NOT NULL, active BOOLEAN NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("membership migration ids are unique");
        Some(plan)
    }
}
