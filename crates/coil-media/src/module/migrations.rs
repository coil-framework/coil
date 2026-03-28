use super::*;

pub(super) fn install_module_migration_plan(module: &MediaModule) -> Option<MigrationPlan> {
    let owner = MigrationOwner::Module(module.name().to_string());
    let mut plan = MigrationPlan::new();

    plan.insert(media_libraries_step(owner.clone()))
        .expect("media migration ids are unique");
    plan.insert(media_assets_step(owner.clone()))
        .expect("media migration ids are unique");
    plan.insert(media_derivatives_step(owner))
        .expect("media migration ids are unique");

    Some(plan)
}

fn media_libraries_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("media_libraries").expect("constant migration id is valid"),
        owner,
        10,
        "Create media-library and folder policy storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS media_libraries (id TEXT PRIMARY KEY, name TEXT NOT NULL, default_policy TEXT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}

fn media_assets_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("media_assets").expect("constant migration id is valid"),
        owner,
        20,
        "Create managed media asset and revision storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS media_assets (id TEXT PRIMARY KEY, library_id TEXT NOT NULL, slug TEXT NOT NULL, status TEXT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}

fn media_derivatives_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("media_derivatives").expect("constant migration id is valid"),
        owner,
        30,
        "Create media derivative and sync backlog storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS media_derivatives (id TEXT PRIMARY KEY, asset_id TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}
