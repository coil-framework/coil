use super::*;

pub(super) fn install_module_migration_plan(module: &MembershipsModule) -> Option<MigrationPlan> {
    let owner = MigrationOwner::Module(module.name().to_string());
    let mut plan = MigrationPlan::new();

    plan.insert(membership_tiers_step(owner.clone()))
        .expect("membership migration ids are unique");
    plan.insert(membership_subscriptions_step(owner.clone()))
        .expect("membership migration ids are unique");
    plan.insert(membership_entitlements_step(owner))
        .expect("membership migration ids are unique");

    Some(plan)
}

fn membership_tiers_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("membership_tiers").expect("constant migration id is valid"),
        owner,
        10,
        "Create membership tier and benefit storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS membership_tiers (id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL)",
    )
    .expect("constant migration statement is valid")
}

fn membership_subscriptions_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("membership_subscriptions").expect("constant migration id is valid"),
        owner,
        20,
        "Create subscription lifecycle and renewal state storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS membership_subscriptions (id TEXT PRIMARY KEY, tier_id TEXT NOT NULL, status TEXT NOT NULL, renews_at BIGINT)",
    )
    .expect("constant migration statement is valid")
}

fn membership_entitlements_step(owner: MigrationOwner) -> MigrationStep {
    MigrationStep::new(
        MigrationId::new("membership_entitlements").expect("constant migration id is valid"),
        owner,
        30,
        "Create entitlement grant and revocation audit storage",
    )
    .expect("constant migration step is valid")
    .with_statement(
        "CREATE TABLE IF NOT EXISTS membership_entitlements (id TEXT PRIMARY KEY, subscription_id TEXT NOT NULL, entitlement_key TEXT NOT NULL, active BOOLEAN NOT NULL)",
    )
    .expect("constant migration statement is valid")
}
