use std::fmt;

use crate::{
    CompiledStatement, DataModelError, DataRuntime, DataValue, MigrationId, quote_identifier,
    require_non_empty,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationOwner {
    Core,
    Module(String),
    CustomerApp(String),
    AuthPackage(String),
}

impl fmt::Display for MigrationOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => f.write_str("core"),
            Self::Module(module) => write!(f, "module:{module}"),
            Self::CustomerApp(app) => write!(f, "customer_app:{app}"),
            Self::AuthPackage(package) => write!(f, "auth_package:{package}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationStep {
    pub id: MigrationId,
    pub owner: MigrationOwner,
    pub order: u32,
    pub description: String,
    pub online_safe: bool,
    pub statements: Vec<String>,
}

impl MigrationStep {
    pub fn new(
        id: MigrationId,
        owner: MigrationOwner,
        order: u32,
        description: impl Into<String>,
    ) -> Result<Self, DataModelError> {
        Ok(Self {
            id,
            owner,
            order,
            description: require_non_empty("migration_description", description.into())?,
            online_safe: true,
            statements: Vec::new(),
        })
    }

    pub fn blocking(mut self) -> Self {
        self.online_safe = false;
        self
    }

    pub fn with_statement(mut self, sql: impl Into<String>) -> Result<Self, DataModelError> {
        self.statements
            .push(require_non_empty("migration_statement", sql.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationPlan {
    steps: Vec<MigrationStep>,
}

impl MigrationPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, step: MigrationStep) -> Result<(), DataModelError> {
        if self
            .steps
            .iter()
            .any(|existing| existing.owner == step.owner && existing.id == step.id)
        {
            return Err(DataModelError::DuplicateMigration {
                owner: step.owner.to_string(),
                migration_id: step.id.to_string(),
            });
        }

        self.steps.push(step);
        self.steps.sort_by(|left, right| {
            owner_rank(&left.owner)
                .cmp(&owner_rank(&right.owner))
                .then(left.order.cmp(&right.order))
                .then(left.id.as_str().cmp(right.id.as_str()))
        });
        Ok(())
    }

    pub fn ordered_steps(&self) -> &[MigrationStep] {
        &self.steps
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationRegistry {
    plan: MigrationPlan,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, plan: &MigrationPlan) -> Result<(), DataModelError> {
        for step in plan.ordered_steps() {
            self.plan.insert(step.clone())?;
        }
        Ok(())
    }

    pub fn composed_plan(&self) -> &MigrationPlan {
        &self.plan
    }

    pub fn compile_apply_batch(
        &self,
        runtime: &DataRuntime,
    ) -> Result<CompiledMigrationBatch, DataModelError> {
        let mut statements = Vec::new();
        let migrations_table =
            quote_identifier(&format!("{}.{}", runtime.schema, runtime.migrations_table));
        statements.push(CompiledStatement {
            sql: format!(
                "CREATE TABLE IF NOT EXISTS {migrations_table} (owner TEXT NOT NULL, migration_id TEXT NOT NULL, description TEXT NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), PRIMARY KEY (owner, migration_id))"
            ),
            bind_values: Vec::new(),
        });

        for step in self.plan.ordered_steps() {
            if step.statements.is_empty() {
                return Err(DataModelError::MissingMigrationStatements {
                    migration_id: step.id.to_string(),
                });
            }

            for sql in &step.statements {
                statements.push(CompiledStatement {
                    sql: sql.clone(),
                    bind_values: Vec::new(),
                });
            }

            statements.push(CompiledStatement {
                sql: format!(
                    "INSERT INTO {migrations_table} (owner, migration_id, description) VALUES ($1, $2, $3) ON CONFLICT (owner, migration_id) DO NOTHING"
                ),
                bind_values: vec![
                    DataValue::String(step.owner.to_string()),
                    DataValue::String(step.id.to_string()),
                    DataValue::String(step.description.clone()),
                ],
            });
        }

        Ok(CompiledMigrationBatch { statements })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledMigrationBatch {
    pub statements: Vec<CompiledStatement>,
}

fn owner_rank(owner: &MigrationOwner) -> u8 {
    match owner {
        MigrationOwner::Core => 0,
        MigrationOwner::Module(_) => 1,
        MigrationOwner::AuthPackage(_) => 2,
        MigrationOwner::CustomerApp(_) => 3,
    }
}
