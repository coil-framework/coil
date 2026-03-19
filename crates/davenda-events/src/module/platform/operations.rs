use super::*;

pub(super) fn module_migrations() -> Vec<MigrationContract> {
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

pub(super) fn jobs() -> Vec<JobContract> {
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

pub(super) fn event_subscriptions() -> Vec<EventSubscription> {
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

pub(super) fn module_behaviors() -> Vec<ModuleBehavior> {
    vec![
        ModuleBehavior::CacheInvalidation,
        ModuleBehavior::LocalizedContent,
        ModuleBehavior::SeoMetadata,
        ModuleBehavior::JsonLd,
        ModuleBehavior::AccessibleAdminUi,
        ModuleBehavior::AsyncJobs,
    ]
}

pub(super) fn extension_slots() -> Vec<ExtensionSlotDescriptor> {
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
