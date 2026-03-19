use super::*;

pub(super) fn register_module_services(
    module: &EventsModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name.clone(),
        "module.events.content",
        "Event content, discoverability, SEO metadata, and public page composition",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.events.slots",
        "Timeslots, capacity rules, and session scheduling",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.events.reservations",
        "Reservation holds, expiry handling, and waitlist promotion",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.events.bookings",
        "Confirmed bookings, cancellations, and booking lifecycle state",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.events.waitlists",
        "Waitlist queue management and promotion workflows",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.events.check_in",
        "Operator check-in workflows for attended events",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.events.admin",
        "Event admin resources, slot operations, and booking review",
    )
}
