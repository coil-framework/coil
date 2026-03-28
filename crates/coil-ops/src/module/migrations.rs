use super::*;

pub(super) fn install_module_migration_plan(module: &OpsModule) -> Option<MigrationPlan> {
    let owner = MigrationOwner::Module(module.name().to_string());
    let mut plan = MigrationPlan::new();

    plan.insert(ops_search_step(owner.clone()))
        .expect("ops migration ids are unique");
    plan.insert(ops_reports_step(owner.clone()))
        .expect("ops migration ids are unique");
    plan.insert(ops_bulk_step(owner))
        .expect("ops migration ids are unique");

    Some(plan)
}

fn ops_search_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("ops_search").expect("constant migration id is valid"),
        owner,
        10,
        "Create search projection and rebuild cursor storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS ops_search_projection (id TEXT PRIMARY KEY, document_type TEXT NOT NULL, visibility TEXT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}

fn ops_reports_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("ops_reports").expect("constant migration id is valid"),
        owner,
        20,
        "Create report definition and export artifact storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS ops_reports (id TEXT PRIMARY KEY, format TEXT NOT NULL, output_path TEXT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}

fn ops_bulk_step(owner: MigrationOwner) -> MigrationStep {
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
    .expect("constant migration statement is valid")
}
