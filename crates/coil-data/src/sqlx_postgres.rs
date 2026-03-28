use std::collections::BTreeSet;
use std::str::FromStr;

use coil_config::DatabaseDriver;
use sqlx::postgres::{PgArguments, PgConnectOptions, PgPoolOptions};
use sqlx::{Column, Pool, Postgres, Row};

use crate::{
    CompiledMigrationBatch, CompiledStatement, CompiledTransaction, DataModelError, DataRuntime,
    DataValue, quote_identifier,
};

#[derive(Debug, Clone)]
pub struct PostgresDataClient {
    pub runtime: DataRuntime,
    pub connection_url: String,
    pub pool: Pool<Postgres>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementExecution {
    pub rows_affected: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExecution {
    pub rows_returned: usize,
    pub projected_columns: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionExecution {
    pub statements_executed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationBatchExecution {
    pub statements_executed: usize,
}

impl PostgresDataClient {
    pub(crate) fn connect_lazy(runtime: &DataRuntime) -> Result<Self, DataModelError> {
        if runtime.driver != DatabaseDriver::Postgres {
            return Err(DataModelError::UnsupportedSqlxDriver {
                driver: runtime.driver,
            });
        }

        let connection_url = runtime.resolve_connection_url()?;
        let options = PgConnectOptions::from_str(&connection_url).map_err(|error| {
            DataModelError::InvalidConnectionUrl {
                reason: error.to_string(),
            }
        })?;
        let pool = PgPoolOptions::new()
            .min_connections(u32::from(runtime.pool.min_connections))
            .max_connections(u32::from(runtime.pool.max_connections))
            .acquire_timeout(runtime.pool.statement_timeout)
            .connect_lazy_with(options);

        Ok(Self {
            runtime: runtime.clone(),
            connection_url,
            pool,
        })
    }

    pub async fn ping(&self) -> Result<(), DataModelError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;
        Ok(())
    }

    pub async fn execute_statement(
        &self,
        statement: &CompiledStatement,
    ) -> Result<StatementExecution, DataModelError> {
        self.apply_statement_timeout().await?;
        let result = bind_query(sqlx::query(&statement.sql), &statement.bind_values)?
            .execute(&self.pool)
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;

        Ok(StatementExecution {
            rows_affected: result.rows_affected(),
        })
    }

    pub async fn execute_query(
        &self,
        query: &crate::CompiledQuery,
    ) -> Result<QueryExecution, DataModelError> {
        self.apply_statement_timeout().await?;
        let rows = bind_query(sqlx::query(&query.sql), &query.bind_values)?
            .fetch_all(&self.pool)
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;

        let projected_columns = rows
            .first()
            .map(|row| {
                row.columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect()
            })
            .unwrap_or_default();

        Ok(QueryExecution {
            rows_returned: rows.len(),
            projected_columns,
        })
    }

    pub async fn execute_transaction(
        &self,
        transaction: &CompiledTransaction,
    ) -> Result<TransactionExecution, DataModelError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;

        sqlx::query(&format!(
            "SET LOCAL statement_timeout = {}",
            self.runtime.pool.statement_timeout.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        for statement in &transaction.statements {
            bind_query(sqlx::query(&statement.sql), &statement.bind_values)?
                .execute(&mut *tx)
                .await
                .map_err(|error| DataModelError::Sqlx {
                    reason: error.to_string(),
                })?;
        }

        tx.commit().await.map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        Ok(TransactionExecution {
            statements_executed: transaction.statements.len(),
        })
    }

    pub async fn apply_migrations(
        &self,
        batch: &CompiledMigrationBatch,
    ) -> Result<MigrationBatchExecution, DataModelError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|error| DataModelError::Sqlx {
                reason: error.to_string(),
            })?;

        sqlx::query(&format!(
            "SET LOCAL statement_timeout = {}",
            self.runtime.pool.statement_timeout.as_millis()
        ))
        .execute(&mut *tx)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        for statement in &batch.statements {
            bind_query(sqlx::query(&statement.sql), &statement.bind_values)?
                .execute(&mut *tx)
                .await
                .map_err(|error| DataModelError::Sqlx {
                    reason: error.to_string(),
                })?;
        }

        tx.commit().await.map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        Ok(MigrationBatchExecution {
            statements_executed: batch.statements.len(),
        })
    }

    pub async fn applied_migration_keys(
        &self,
    ) -> Result<BTreeSet<(String, String)>, DataModelError> {
        let migrations_table = quote_identifier(&format!(
            "{}.{}",
            self.runtime.schema, self.runtime.migrations_table
        ));
        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS {migrations_table} (owner TEXT NOT NULL, migration_id TEXT NOT NULL, description TEXT NOT NULL, applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), PRIMARY KEY (owner, migration_id))"
        ))
        .execute(&self.pool)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        let rows = sqlx::query(&format!(
            "SELECT owner, migration_id FROM {migrations_table} ORDER BY owner ASC, migration_id ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;

        Ok(rows
            .into_iter()
            .map(|row| (row.get("owner"), row.get("migration_id")))
            .collect())
    }

    async fn apply_statement_timeout(&self) -> Result<(), DataModelError> {
        sqlx::query(&format!(
            "SET statement_timeout = {}",
            self.runtime.pool.statement_timeout.as_millis()
        ))
        .execute(&self.pool)
        .await
        .map_err(|error| DataModelError::Sqlx {
            reason: error.to_string(),
        })?;
        Ok(())
    }
}

pub(crate) fn bind_query<'q>(
    mut query: sqlx::query::Query<'q, Postgres, PgArguments>,
    values: &[DataValue],
) -> Result<sqlx::query::Query<'q, Postgres, PgArguments>, DataModelError> {
    for value in values {
        query = match value {
            DataValue::String(value) => query.bind(value.clone()),
            DataValue::Int(value) => query.bind(*value),
            DataValue::UInt(value) => {
                let value = i64::try_from(*value)
                    .map_err(|_| DataModelError::UnsupportedUnsignedBindValue { value: *value })?;
                query.bind(value)
            }
            DataValue::Bool(value) => query.bind(*value),
        };
    }

    Ok(query)
}
