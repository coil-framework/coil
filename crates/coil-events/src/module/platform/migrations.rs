use super::*;

pub(super) fn install_module_migration_plan(module: &EventsModule) -> Option<MigrationPlan> {
    let owner = MigrationOwner::Module(module.name.clone());
    let mut plan = MigrationPlan::new();

    plan.insert(events_catalog_step(owner.clone()))
        .expect("event migration ids are unique");
    plan.insert(event_slots_step(owner.clone()))
        .expect("event migration ids are unique");
    plan.insert(event_bookings_step(owner.clone()))
        .expect("event migration ids are unique");
    plan.insert(event_publications_step(owner))
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
        "CREATE TABLE IF NOT EXISTS events_catalog (id TEXT PRIMARY KEY, slug TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, starts_at TEXT NOT NULL, ends_at TEXT, summary TEXT, hero_asset TEXT, source_system TEXT, source_key TEXT UNIQUE, import_batch_id TEXT, fingerprint TEXT NOT NULL, published_at BIGINT, updated_at BIGINT NOT NULL)",
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

fn event_publications_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("event_publications").expect("constant migration id is valid"),
        owner,
        40,
        "Create site-scoped event publication and booking ownership storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS event_publications (event_id TEXT NOT NULL REFERENCES events_catalog(id), site_id TEXT NOT NULL, locale TEXT NOT NULL, summary TEXT NOT NULL, is_published BOOLEAN NOT NULL DEFAULT FALSE, updated_at BIGINT NOT NULL, PRIMARY KEY (event_id, site_id, locale))",
    )
    .expect("constant migration statement is valid")
    .with_statement(
        "CREATE INDEX IF NOT EXISTS event_publications_lookup ON event_publications (site_id, locale, is_published, event_id)",
    )
    .expect("constant migration statement is valid")
    .with_statement(
        "ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS site_id TEXT",
    )
    .expect("constant migration statement is valid")
    .with_statement(
        "ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS session_id TEXT",
    )
    .expect("constant migration statement is valid")
    .with_statement(
        "ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS principal_id TEXT",
    )
    .expect("constant migration statement is valid")
    .with_statement(
        "ALTER TABLE event_bookings ADD COLUMN IF NOT EXISTS created_at BIGINT",
    )
    .expect("constant migration statement is valid")
}
