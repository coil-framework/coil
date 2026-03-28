use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationPlanOwner {
    Module(String),
    AuthPackage(String),
    CustomerApp(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlanEntry {
    pub owner: MigrationPlanOwner,
    pub step_id: Option<String>,
    pub order: u32,
    pub description: String,
    pub online_safe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationPlanSummary {
    entries: Vec<MigrationPlanEntry>,
}

impl MigrationPlanSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: MigrationPlanEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|left, right| {
            migration_owner_rank(&left.owner)
                .cmp(&migration_owner_rank(&right.owner))
                .then(left.order.cmp(&right.order))
                .then(
                    left.step_id
                        .as_deref()
                        .unwrap_or("")
                        .cmp(right.step_id.as_deref().unwrap_or("")),
                )
        });
    }

    pub fn entries(&self) -> &[MigrationPlanEntry] {
        &self.entries
    }

    pub fn command_report(&self) -> Result<CommandReport, AppModelError> {
        let mut report = CommandReport::new(
            ["migrate", "plan"],
            "Composed module, auth-package, and customer-app migration plan",
        )?
        .with_columns(["owner", "step", "order", "online_safe", "description"])?;
        if self.entries.iter().any(|entry| !entry.online_safe) {
            report = report.with_status(ReportStatus::Warning);
        }

        for entry in &self.entries {
            report.push_row(
                ReportRow::new()
                    .with_cell("owner", migration_owner_label(&entry.owner))?
                    .with_cell(
                        "step",
                        entry
                            .step_id
                            .clone()
                            .unwrap_or_else(|| "version-check".to_string()),
                    )?
                    .with_cell("order", entry.order.to_string())?
                    .with_cell("online_safe", entry.online_safe.to_string())?
                    .with_cell("description", entry.description.clone())?,
            );
        }

        Ok(report)
    }
}

pub(crate) fn build_migration_summary(
    manifest: &CustomerAppManifest,
    auth_package_name: String,
    modules: &[Box<dyn PlatformModule>],
) -> MigrationPlanSummary {
    let mut summary = MigrationPlanSummary::new();
    let installed_modules = manifest
        .modules
        .iter()
        .map(|module| module.id.to_string())
        .collect::<BTreeSet<_>>();

    for module in modules {
        let module_manifest = module.manifest();
        if !installed_modules.contains(&module_manifest.name) {
            continue;
        }

        if let Some(plan) = module.install_migration_plan() {
            append_migration_plan(&mut summary, &plan);
        }
    }

    summary.push(MigrationPlanEntry {
        owner: MigrationPlanOwner::AuthPackage(auth_package_name.clone()),
        step_id: None,
        order: 0,
        description: format!(
            "validate auth package `{auth_package_name}` schema, model, and capability bindings before release"
        ),
        online_safe: true,
    });

    for migration in &manifest.customer_migrations {
        summary.push(MigrationPlanEntry {
            owner: MigrationPlanOwner::CustomerApp(manifest.id.to_string()),
            step_id: None,
            order: migration.order,
            description: migration.description.clone(),
            online_safe: true,
        });
    }

    summary
}

fn append_migration_plan(summary: &mut MigrationPlanSummary, plan: &MigrationPlan) {
    for step in plan.ordered_steps() {
        let owner = match &step.owner {
            MigrationOwner::Module(module) => MigrationPlanOwner::Module(module.clone()),
            MigrationOwner::AuthPackage(package) => {
                MigrationPlanOwner::AuthPackage(package.clone())
            }
            MigrationOwner::CustomerApp(app_id) => MigrationPlanOwner::CustomerApp(app_id.clone()),
            MigrationOwner::Core => continue,
        };

        summary.push(MigrationPlanEntry {
            owner,
            step_id: Some(step.id.to_string()),
            order: step.order,
            description: step.description.clone(),
            online_safe: step.online_safe,
        });
    }
}

fn migration_owner_rank(owner: &MigrationPlanOwner) -> u8 {
    match owner {
        MigrationPlanOwner::Module(_) => 1,
        MigrationPlanOwner::AuthPackage(_) => 2,
        MigrationPlanOwner::CustomerApp(_) => 3,
    }
}

fn migration_owner_label(owner: &MigrationPlanOwner) -> String {
    match owner {
        MigrationPlanOwner::Module(module) => format!("module:{module}"),
        MigrationPlanOwner::AuthPackage(package) => format!("auth:{package}"),
        MigrationPlanOwner::CustomerApp(app_id) => format!("customer_app:{app_id}"),
    }
}
