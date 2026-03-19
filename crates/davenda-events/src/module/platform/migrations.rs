use super::*;

pub(super) fn install_module_migration_plan(module: &EventsModule) -> Option<MigrationPlan> {
    let owner = MigrationOwner::Module(module.name.clone());
    let mut plan = MigrationPlan::new();

    plan.insert(events_catalog_step(owner.clone()))
        .expect("event migration ids are unique");
    plan.insert(event_slots_step(owner.clone()))
        .expect("event migration ids are unique");
    plan.insert(event_bookings_step(owner))
        .expect("event migration ids are unique");

    Some(plan)
}

fn events_catalog_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("events_catalog").expect("constant migration id is valid"),
        owner,
        10,
        "Create event catalog and publication storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS events_catalog (id TEXT PRIMARY KEY, slug TEXT NOT NULL, status TEXT NOT NULL, published_at BIGINT)",
    )
    .expect("constant migration statement is valid")
}

fn event_slots_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("event_slots").expect("constant migration id is valid"),
        owner,
        20,
        "Create event slot and capacity storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS event_slots (id TEXT PRIMARY KEY, event_id TEXT NOT NULL, starts_at BIGINT NOT NULL, capacity BIGINT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}

fn event_bookings_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("event_bookings").expect("constant migration id is valid"),
        owner,
        30,
        "Create booking, reservation, waitlist, and check-in storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS event_bookings (id TEXT PRIMARY KEY, slot_id TEXT NOT NULL, status TEXT NOT NULL, checked_in_at BIGINT)",
    )
    .expect("constant migration statement is valid")
}
