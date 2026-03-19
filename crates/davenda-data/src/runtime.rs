use std::time::Duration;

use davenda_config::{DatabaseConfig, SecretRef};

use crate::{
    CompiledMigrationBatch, CompiledQuery, CompiledStatement, CompiledTransaction, DataModelError,
    DataValue, MigrationRegistry, MutationSpec, PostgresDataClient, QuerySpec, RepositorySpec,
    TransactionPlan, quote_identifier, require_non_empty,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPoolProfile {
    pub min_connections: u16,
    pub max_connections: u16,
    pub statement_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRuntime {
    pub driver: davenda_config::DatabaseDriver,
    pub connection_secret_ref: Option<SecretRef>,
    pub connection_secret: Option<String>,
    pub schema: String,
    pub migrations_table: String,
    pub pool: ConnectionPoolProfile,
}

impl DataRuntime {
    pub fn from_config(config: &DatabaseConfig) -> Result<Self, DataModelError> {
        if config.max_connections == 0 || config.min_connections > config.max_connections {
            return Err(DataModelError::InvalidPoolSizing {
                min_connections: config.min_connections,
                max_connections: config.max_connections,
            });
        }

        if config.statement_timeout_secs == 0 {
            return Err(DataModelError::InvalidStatementTimeout);
        }

        Ok(Self {
            driver: config.driver,
            connection_secret_ref: config.url.clone(),
            connection_secret: config.url.as_ref().map(|secret| secret.redacted()),
            schema: require_non_empty("database_schema", config.schema.clone())?,
            migrations_table: require_non_empty(
                "database_migrations_table",
                config.migrations_table.clone(),
            )?,
            pool: ConnectionPoolProfile {
                min_connections: config.min_connections,
                max_connections: config.max_connections,
                statement_timeout: Duration::from_secs(config.statement_timeout_secs),
            },
        })
    }

    pub fn resolve_connection_url(&self) -> Result<String, DataModelError> {
        match self.connection_secret_ref.as_ref() {
            Some(SecretRef::Env { var }) => std::env::var(var)
                .map_err(|_| DataModelError::MissingConnectionSecretEnv { var: var.clone() }),
            Some(secret_ref) => Err(DataModelError::UnsupportedSecretRef {
                secret_ref: secret_ref.redacted(),
            }),
            None => self
                .connection_secret
                .clone()
                .ok_or(DataModelError::MissingConnectionSecret),
        }
    }

    pub fn with_resolved_connection_url(&self, connection_url: impl Into<String>) -> Self {
        let mut runtime = self.clone();
        runtime.connection_secret_ref = None;
        runtime.connection_secret = Some(connection_url.into());
        runtime
    }

    pub fn connect_lazy_postgres(&self) -> Result<PostgresDataClient, DataModelError> {
        PostgresDataClient::connect_lazy(self)
    }

    pub fn compile_query(
        &self,
        repository: &RepositorySpec,
        spec: &QuerySpec,
    ) -> Result<CompiledQuery, DataModelError> {
        repository.compile_query(spec)
    }

    pub fn compile_transaction(
        &self,
        plan: &TransactionPlan,
        mutations: &[MutationSpec],
    ) -> Result<CompiledTransaction, DataModelError> {
        if plan.writes.len() != mutations.len() {
            return Err(DataModelError::TransactionWriteCountMismatch {
                expected: plan.writes.len(),
                actual: mutations.len(),
            });
        }

        let mut statements = Vec::new();
        let mut bind_index = 1;
        for mutation in mutations {
            let compiled = mutation.compile(bind_index)?;
            bind_index += compiled.bind_values.len();
            statements.push(compiled);
        }

        let jobs_table = quote_identifier(&format!("{}.job_outbox", self.schema));
        for job in &plan.after_commit_jobs {
            statements.push(CompiledStatement {
                sql: format!(
                    "INSERT INTO {jobs_table} (transaction_name, job_name) VALUES ($1, $2)"
                ),
                bind_values: vec![
                    DataValue::String(plan.name.clone()),
                    DataValue::String(job.clone()),
                ],
            });
        }

        let events_table = quote_identifier(&format!("{}.event_outbox", self.schema));
        for event in &plan.after_commit_events {
            statements.push(CompiledStatement {
                sql: format!(
                    "INSERT INTO {events_table} (transaction_name, event_name) VALUES ($1, $2)"
                ),
                bind_values: vec![
                    DataValue::String(plan.name.clone()),
                    DataValue::String(event.clone()),
                ],
            });
        }

        Ok(CompiledTransaction {
            begin_sql: "BEGIN".to_string(),
            commit_sql: "COMMIT".to_string(),
            statements: std::iter::once(CompiledStatement {
                sql: format!("SET TRANSACTION ISOLATION LEVEL {}", plan.isolation),
                bind_values: Vec::new(),
            })
            .chain(statements)
            .collect(),
        })
    }

    pub fn compile_migrations(
        &self,
        registry: &MigrationRegistry,
    ) -> Result<CompiledMigrationBatch, DataModelError> {
        registry.compile_apply_batch(self)
    }
}
