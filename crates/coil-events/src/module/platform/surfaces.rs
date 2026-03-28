use super::*;

pub(super) fn route_surfaces() -> Vec<RouteSurface> {
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

pub(super) fn integration_points() -> Vec<IntegrationPoint> {
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

pub(super) fn http_surfaces() -> Vec<HttpSurfaceContribution> {
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
