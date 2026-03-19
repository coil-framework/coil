use super::super::support::{default_retry_policy, events_waitlist_repository};
use super::*;

pub(super) fn build_manifest(module: &EventsModule) -> ModuleManifest {
    ModuleManifest::new(module.name.clone())
        .with_required_capabilities(required_capabilities())
        .with_optional_capabilities(optional_capabilities())
        .with_config_namespace(module.config_namespace.clone())
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
        .with_admin_resources(module.admin_resources.clone())
        .with_search_contributions(search_contributions())
        .with_report_definitions(report_definitions())
        .with_bulk_operations(bulk_operations())
        .with_data_repositories(vec![events_waitlist_repository()])
        .with_http_surfaces(http_surfaces())
}

fn required_capabilities() -> Vec<Capability> {
    vec![
        Capability::EventsEventPublish,
        Capability::EventsSlotManage,
        Capability::EventsBookingCreate,
        Capability::EventsBookingCheckIn,
    ]
}

fn optional_capabilities() -> Vec<Capability> {
    vec![
        Capability::AdminShellAccess,
        Capability::CmsPageRead,
        Capability::SeoMetadataEdit,
        Capability::I18nTranslationEdit,
        Capability::MembershipSubscriptionManage,
        Capability::AssetRead,
        Capability::CheckoutSessionCreate,
        Capability::OrderRead,
    ]
}

fn capability_contracts() -> Vec<CapabilityContract> {
    vec![
        CapabilityContract::required(Capability::EventsEventPublish, ["event"]),
        CapabilityContract::required(Capability::EventsSlotManage, ["event_slot"]),
        CapabilityContract::required(Capability::EventsBookingCreate, ["booking", "event_slot"]),
        CapabilityContract::required(Capability::EventsBookingCheckIn, ["booking"]),
        CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
        CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
        CapabilityContract::optional(Capability::SeoMetadataEdit, ["event"]),
        CapabilityContract::optional(Capability::I18nTranslationEdit, ["event"]),
        CapabilityContract::optional(
            Capability::MembershipSubscriptionManage,
            ["subscription", "membership_tier"],
        ),
        CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
        CapabilityContract::optional(Capability::CheckoutSessionCreate, ["storefront"]),
        CapabilityContract::optional(Capability::OrderRead, ["order"]),
    ]
}

fn module_dependencies() -> Vec<ModuleDependency> {
    vec![
        ModuleDependency::optional(
            "admin",
            "Events contributes booking, slot, and check-in resources to the shared admin shell when installed",
        ),
        ModuleDependency::optional(
            "cms",
            "Event pages and discoverability can compose into CMS-driven storefront content",
        ),
        ModuleDependency::optional(
            "commerce",
            "Paid bookings can bridge into checkout and order workflows when commerce is installed",
        ),
        ModuleDependency::optional(
            "memberships",
            "Membership tiers can gate event eligibility and booking access rules",
        ),
    ]
}

fn core_service_dependencies() -> Vec<CoreServiceDependency> {
    vec![
        CoreServiceDependency::Auth,
        CoreServiceDependency::Data,
        CoreServiceDependency::Cache,
        CoreServiceDependency::Jobs,
        CoreServiceDependency::I18n,
        CoreServiceDependency::Seo,
        CoreServiceDependency::Template,
        CoreServiceDependency::Observability,
    ]
}

fn module_migrations() -> Vec<MigrationContract> {
    vec![
        MigrationContract::new(
            "events.catalog",
            10,
            "Creates event content, discoverability, and publication state tables",
        ),
        MigrationContract::new(
            "events.slots",
            20,
            "Creates event-slot capacity, timing, and reservation state tables",
        ),
        MigrationContract::new(
            "events.bookings",
            30,
            "Creates booking, waitlist, and check-in lifecycle tables",
        ),
    ]
}

fn route_surfaces() -> Vec<RouteSurface> {
    vec![
        RouteSurface::new("events.list", RouteSurfaceKind::FrontendPage, "/events").localized(),
        RouteSurface::new(
            "events.detail",
            RouteSurfaceKind::FrontendPage,
            "/events/{event_slug}",
        )
        .localized(),
        RouteSurface::new(
            "events.book",
            RouteSurfaceKind::FrontendAction,
            "/events/{event_slug}/book",
        )
        .gated_by(Capability::EventsBookingCreate),
        RouteSurface::new(
            "events.admin.index",
            RouteSurfaceKind::AdminPage,
            "/admin/events",
        )
        .gated_by(Capability::EventsEventPublish),
        RouteSurface::new(
            "events.admin.bookings",
            RouteSurfaceKind::AdminPage,
            "/admin/events/bookings",
        )
        .gated_by(Capability::EventsBookingCreate),
        RouteSurface::new(
            "events.admin.check-in",
            RouteSurfaceKind::AdminPage,
            "/admin/events/check-in",
        )
        .gated_by(Capability::EventsBookingCheckIn),
    ]
}

fn jobs() -> Vec<JobContract> {
    vec![
        JobContract::new(
            "events.reservation-expiry",
            JobTriggerKind::Scheduled,
            true,
            "Releases expired reservation holds and promotes waitlisted attendees when capacity returns",
        ),
        JobContract::new(
            "events.waitlist-promotion",
            JobTriggerKind::DomainEvent,
            true,
            "Promotes waitlist entries after cancellations or released holds",
        ),
        JobContract::new(
            "events.reminders",
            JobTriggerKind::Scheduled,
            true,
            "Schedules reminder and attendance preparation notifications for upcoming bookings",
        ),
    ]
}

fn event_subscriptions() -> Vec<EventSubscription> {
    vec![
        EventSubscription::new(
            "commerce.order.paid",
            Some("events.waitlist-promotion"),
            "Allows paid-booking confirmation flows to reconcile held reservations into confirmed bookings",
        ),
        EventSubscription::new(
            "membership.subscription.activated",
            Some("events.reminders"),
            "Refreshes member-only eligibility and upcoming-event communication windows after subscription changes",
        ),
    ]
}

fn integration_points() -> Vec<IntegrationPoint> {
    vec![
        IntegrationPoint::new(
            IntegrationKind::FrontendRendering,
            "events.pages",
            "Provides public event discovery, detail pages, and booking entry points",
        ),
        IntegrationPoint::new(
            IntegrationKind::AdminWorkflow,
            "events.check-in",
            "Adds check-in, booking review, and slot operations to the shared admin shell",
        ),
        IntegrationPoint::new(
            IntegrationKind::SeoMetadata,
            "events.head",
            "Emits event metadata and rich-result schema for discoverable event pages",
        ),
        IntegrationPoint::new(
            IntegrationKind::JsonLd,
            "events.schema",
            "Supplies JSON-LD for event pages and schedule-rich discovery surfaces",
        ),
        IntegrationPoint::new(
            IntegrationKind::SearchIndex,
            "events.index",
            "Publishes searchable public event and operator booking visibility data",
        ),
        IntegrationPoint::new(
            IntegrationKind::CommerceBridge,
            "events.paid-bookings",
            "Bridges optional paid-booking flows into checkout and order outcomes",
        ),
    ]
}

fn module_behaviors() -> Vec<ModuleBehavior> {
    vec![
        ModuleBehavior::CacheInvalidation,
        ModuleBehavior::LocalizedContent,
        ModuleBehavior::SeoMetadata,
        ModuleBehavior::JsonLd,
        ModuleBehavior::AccessibleAdminUi,
        ModuleBehavior::AsyncJobs,
    ]
}

fn extension_slots() -> Vec<ExtensionSlotDescriptor> {
    vec![
        ExtensionSlotDescriptor::new(
            ExtensionSlotKind::AdminWidget,
            "events.booking.summary",
            "Allows bounded widgets to enrich booking and attendance operations",
        ),
        ExtensionSlotDescriptor::new(
            ExtensionSlotKind::RenderHook,
            "events.page.render",
            "Allows controlled customer embellishments around event page rendering",
        ),
    ]
}

fn search_contributions() -> Vec<SearchIndexContribution> {
    vec![
        SearchIndexContribution::new(
            "search.events",
            SearchDocumentKind::Event,
            SearchVisibility::Public,
            true,
            vec![
                SearchFieldContribution::new("title", "title", SearchFieldRole::Title, true, true),
                SearchFieldContribution::new(
                    "summary",
                    "summary",
                    SearchFieldRole::Summary,
                    false,
                    true,
                ),
                SearchFieldContribution::new(
                    "location",
                    "location",
                    SearchFieldRole::Facet,
                    true,
                    true,
                ),
            ],
            vec![
                SearchInvalidationRule::new(
                    SearchInvalidationTrigger::Published,
                    "event published",
                ),
                SearchInvalidationRule::new(SearchInvalidationTrigger::Updated, "event updated"),
            ],
            SearchRebuildStrategy::Scheduled {
                interval: Duration::from_secs(3600),
            },
        ),
        SearchIndexContribution::new(
            "search.events.bookings",
            SearchDocumentKind::Booking,
            SearchVisibility::Capability(Capability::EventsBookingCheckIn),
            false,
            vec![
                SearchFieldContribution::new(
                    "attendee",
                    "attendee.display_name",
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
                SearchInvalidationRule::new(SearchInvalidationTrigger::Updated, "booking changed"),
                SearchInvalidationRule::new(SearchInvalidationTrigger::Deleted, "booking deleted"),
            ],
            SearchRebuildStrategy::ManualOnly,
        ),
    ]
}

fn report_definitions() -> Vec<ReportDefinition> {
    vec![ReportDefinition::new(
        "report.events.attendance",
        "Event attendance",
        Some("Attendance and booking-state export for check-in operations".to_string()),
        Capability::EventsBookingCheckIn,
        ReportFormat::Csv,
        ReportSensitivity::Internal,
        ReportDeliveryMode::SignedUrl,
        "reports/events",
        default_retry_policy(),
    )]
}

fn bulk_operations() -> Vec<BulkOperationDefinition> {
    vec![BulkOperationDefinition::new(
        "bulk.events.check-in",
        "Bulk check in bookings",
        Some("Applies audited attendance check-in through retry-safe job execution".to_string()),
        Capability::EventsBookingCheckIn,
        BulkOperationKind::CheckIn,
        BulkOperationScope::Events,
        default_retry_policy(),
        Some(1000),
        true,
    )]
}

fn http_surfaces() -> Vec<HttpSurfaceContribution> {
    vec![
        HttpSurfaceContribution::page(
            "events.list",
            HttpSurfaceArea::Public,
            "/events",
            "events/list",
        )
        .localized(),
        HttpSurfaceContribution::page(
            "events.detail",
            HttpSurfaceArea::Public,
            "/events/{event_slug}",
            "events/detail",
        )
        .localized(),
        HttpSurfaceContribution::json(
            "events.book",
            HttpSurfaceMethod::Post,
            HttpSurfaceArea::Public,
            "/events/{event_slug}/book",
            202,
            BTreeMap::from([("status".to_string(), "queued".to_string())]),
        )
        .gated_by(Capability::EventsBookingCreate),
        HttpSurfaceContribution::page(
            "events.admin.index",
            HttpSurfaceArea::Admin,
            "/admin/events",
            "events/admin/index",
        )
        .gated_by(Capability::EventsEventPublish),
        HttpSurfaceContribution::page(
            "events.admin.bookings",
            HttpSurfaceArea::Admin,
            "/admin/events/bookings",
            "events/admin/bookings",
        )
        .gated_by(Capability::EventsBookingCreate),
        HttpSurfaceContribution::page(
            "events.admin.check-in",
            HttpSurfaceArea::Admin,
            "/admin/events/check-in",
            "events/admin/check-in",
        )
        .gated_by(Capability::EventsBookingCheckIn),
    ]
}
