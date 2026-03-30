use super::*;

pub(super) fn build_manifest(module: &MembershipsModule) -> ModuleManifest {
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
        .with_admin_resources(module.admin_resources().to_vec())
        .with_search_contributions(search_contributions())
        .with_report_definitions(report_definitions())
        .with_http_surfaces(http_surfaces())
}

fn required_capabilities() -> Vec<Capability> {
    vec![
        Capability::MembershipSubscriptionManage,
        Capability::MembershipTierEdit,
    ]
}

fn optional_capabilities() -> Vec<Capability> {
    vec![
        Capability::AdminShellAccess,
        Capability::OrderRead,
        Capability::I18nTranslationEdit,
        Capability::AssetRead,
    ]
}

fn capability_contracts() -> Vec<CapabilityContract> {
    vec![
        CapabilityContract::required(Capability::MembershipSubscriptionManage, ["subscription"]),
        CapabilityContract::required(Capability::MembershipTierEdit, ["membership_tier"]),
        CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
        CapabilityContract::optional(Capability::OrderRead, ["order"]),
        CapabilityContract::optional(Capability::I18nTranslationEdit, ["membership_tier"]),
        CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
    ]
}

fn module_dependencies() -> Vec<ModuleDependency> {
    vec![
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
    ]
}

fn core_service_dependencies() -> Vec<CoreServiceDependency> {
    vec![
        CoreServiceDependency::Auth,
        CoreServiceDependency::Data,
        CoreServiceDependency::Jobs,
        CoreServiceDependency::Observability,
        CoreServiceDependency::I18n,
    ]
}

fn module_migrations() -> Vec<MigrationContract> {
    vec![
        MigrationContract::new(
            "memberships.member_accounts",
            5,
            "Creates member account profile storage for imported and managed account state",
        ),
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
    ]
}

fn route_surfaces() -> Vec<RouteSurface> {
    vec![
        RouteSurface::new(
            "memberships.account.dashboard",
            RouteSurfaceKind::FrontendPage,
            "/account",
        ),
        RouteSurface::new(
            "memberships.account",
            RouteSurfaceKind::FrontendPage,
            "/account/memberships",
        ),
        RouteSurface::new(
            "memberships.account.passes",
            RouteSurfaceKind::FrontendPage,
            "/account/passes",
        ),
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
        RouteSurface::new(
            "memberships.passes",
            RouteSurfaceKind::AdminPage,
            "/admin/memberships/passes",
        )
        .gated_by(Capability::MembershipSubscriptionManage),
    ]
}

fn jobs() -> Vec<JobContract> {
    vec![
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
    ]
}

fn event_subscriptions() -> Vec<EventSubscription> {
    vec![
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
    ]
}

fn integration_points() -> Vec<IntegrationPoint> {
    vec![
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
        IntegrationPoint::new(
            IntegrationKind::FrontendRendering,
            "account.passes",
            "Provides pass-backed access, credit balance, and event-linked entitlement visibility in the customer account",
        ),
    ]
}

fn module_behaviors() -> Vec<ModuleBehavior> {
    vec![
        ModuleBehavior::AccessibleAdminUi,
        ModuleBehavior::AsyncJobs,
        ModuleBehavior::AuditedBulkActions,
    ]
}

fn extension_slots() -> Vec<ExtensionSlotDescriptor> {
    vec![ExtensionSlotDescriptor::new(
        ExtensionSlotKind::AdminWidget,
        "memberships.subscription.summary",
        "Allows customer app widgets to augment subscription detail views with bounded insights",
    )]
}

fn search_contributions() -> Vec<SearchIndexContribution> {
    vec![SearchIndexContribution::new(
        "search.memberships",
        SearchDocumentKind::MembershipSubscription,
        SearchVisibility::Capability(Capability::MembershipSubscriptionManage),
        false,
        vec![
            SearchFieldContribution::new("tier", "tier_name", SearchFieldRole::Title, true, true),
            SearchFieldContribution::new("status", "status", SearchFieldRole::Facet, true, true),
        ],
        vec![
            SearchInvalidationRule::new(SearchInvalidationTrigger::Updated, "subscription changed"),
            SearchInvalidationRule::new(
                SearchInvalidationTrigger::ManualRebuild,
                "membership audit rebuild",
            ),
        ],
        SearchRebuildStrategy::ManualOnly,
    )]
}

fn report_definitions() -> Vec<ReportDefinition> {
    vec![ReportDefinition::new(
        "report.memberships.summary",
        "Subscription summary",
        Some("Lifecycle summary for memberships and renewals".to_string()),
        Capability::MembershipSubscriptionManage,
        ReportFormat::Csv,
        ReportSensitivity::Internal,
        ReportDeliveryMode::SignedUrl,
        "reports/memberships",
        default_retry_policy(),
    )]
}

fn http_surfaces() -> Vec<HttpSurfaceContribution> {
    vec![
        HttpSurfaceContribution::page(
            "memberships.account.dashboard",
            HttpSurfaceArea::Account,
            "/account",
            "account/dashboard",
        ),
        HttpSurfaceContribution::page(
            "memberships.account",
            HttpSurfaceArea::Account,
            "/account/memberships",
            "memberships/account",
        ),
        HttpSurfaceContribution::page(
            "memberships.account.passes",
            HttpSurfaceArea::Account,
            "/account/passes",
            "account/passes",
        ),
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
        HttpSurfaceContribution::page(
            "memberships.passes",
            HttpSurfaceArea::Admin,
            "/admin/memberships/passes",
            "memberships/passes",
        )
        .gated_by(Capability::MembershipSubscriptionManage),
    ]
}
