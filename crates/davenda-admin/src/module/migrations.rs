use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

use super::AdminModule;

pub(super) fn build_migration_plan(module: &AdminModule) -> Option<MigrationPlan> {
    let owner = MigrationOwner::Module(module.name.clone());
    let mut plan = MigrationPlan::new();
    plan.insert(
        MigrationStep::new(
            MigrationId::new("admin_audit_log").expect("constant migration id is valid"),
            owner.clone(),
            10,
            "Create admin audit storage for operator actions",
        )
        .expect("constant migration step is valid")
        .with_statement(
            "CREATE TABLE IF NOT EXISTS admin_audit_log (id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, action TEXT NOT NULL, created_at BIGINT NOT NULL)",
        )
        .expect("constant migration statement is valid"),
    )
    .expect("admin migration ids are unique");
    plan.insert(
        MigrationStep::new(
            MigrationId::new("admin_dashboard_state").expect("constant migration id is valid"),
            owner,
            20,
            "Create dashboard state storage for admin shell preferences",
        )
        .expect("constant migration step is valid")
        .with_statement(
            "CREATE TABLE IF NOT EXISTS admin_dashboard_state (operator_id TEXT PRIMARY KEY, layout_json TEXT NOT NULL, updated_at BIGINT NOT NULL)",
        )
        .expect("constant migration statement is valid"),
    )
    .expect("admin migration ids are unique");
    Some(plan)
}
