use std::sync::OnceLock;

use davenda_data::{DataRuntime, PostgresDataClient};
use sqlx::Row;

use super::*;

#[derive(Debug, Clone)]
pub(super) struct PostgresMetadataAuditStore {
    runtime: DataRuntime,
    client: OnceLock<Result<PostgresDataClient, String>>,
    schema: String,
    initialized: OnceLock<Result<(), String>>,
}

impl PostgresMetadataAuditStore {
    pub(super) fn open(runtime: DataRuntime) -> Self {
        let schema = runtime.schema.clone();
        Self {
            runtime,
            client: OnceLock::new(),
            schema,
            initialized: OnceLock::new(),
        }
    }

    pub(super) fn location_label(&self) -> String {
        format!("{}.metadata_audit_entries", self.schema)
    }

    pub(super) fn insert(&self, record: &MetadataAuditRecord) -> Result<(), String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            sqlx::query(&format!(
                "INSERT INTO {} (recorded_at_unix_seconds, app_id, trace_id, request_id, principal_kind, principal_id, kind) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                table
            ))
            .bind(record.recorded_at_unix_seconds)
            .bind(&record.app_id)
            .bind(&record.trace_id)
            .bind(&record.request_id)
            .bind(&record.principal_kind)
            .bind(&record.principal_id)
            .bind(&record.kind)
            .execute(&client.pool)
            .await
            .map_err(|error| format!("failed to write metadata audit entry: {error}"))?;
            Ok(())
        })
    }

    pub(super) fn count(&self) -> Result<usize, String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table))
                .fetch_one(&client.pool)
                .await
                .map_err(|error| format!("failed to count metadata audit entries: {error}"))?;
            usize::try_from(count)
                .map_err(|_| "metadata audit entry count overflowed usize".to_string())
        })
    }

    pub(super) fn recent(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            let rows = sqlx::query(&format!(
                "SELECT id, recorded_at_unix_seconds, app_id, trace_id, request_id, principal_kind, principal_id, kind FROM {} ORDER BY recorded_at_unix_seconds DESC, id DESC LIMIT $1",
                table
            ))
            .bind(limit as i64)
            .fetch_all(&client.pool)
            .await
            .map_err(|error| format!("failed to query metadata audit entries: {error}"))?;

            let mut records = rows
                .into_iter()
                .map(|row| {
                    Ok(MetadataAuditRecord {
                        id: row.try_get(0).map_err(|error| {
                            format!("failed to decode metadata audit entry id: {error}")
                        })?,
                        recorded_at_unix_seconds: row.try_get(1).map_err(|error| {
                            format!("failed to decode metadata audit timestamp: {error}")
                        })?,
                        app_id: row.try_get(2).map_err(|error| {
                            format!("failed to decode metadata audit app id: {error}")
                        })?,
                        trace_id: row.try_get(3).map_err(|error| {
                            format!("failed to decode metadata audit trace id: {error}")
                        })?,
                        request_id: row.try_get(4).map_err(|error| {
                            format!("failed to decode metadata audit request id: {error}")
                        })?,
                        principal_kind: row.try_get(5).map_err(|error| {
                            format!("failed to decode metadata audit principal kind: {error}")
                        })?,
                        principal_id: row.try_get(6).map_err(|error| {
                            format!("failed to decode metadata audit principal id: {error}")
                        })?,
                        kind: row.try_get(7).map_err(|error| {
                            format!("failed to decode metadata audit kind: {error}")
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            records.reverse();
            Ok(records)
        })
    }

    fn client(&self) -> Result<&PostgresDataClient, String> {
        self.client
            .get_or_init(|| {
                self.runtime
                    .connect_lazy_postgres()
                    .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(|error| error.clone())
    }

    fn ensure_initialized(&self) -> Result<(), String> {
        let schema_ident = quote_identifier(&self.schema);
        self.initialized
            .get_or_init(|| {
                let client = self.client()?.clone();
                run_blocking(async move {
                    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema_ident}"))
                        .execute(&client.pool)
                        .await
                        .map_err(|error| format!("failed to initialize metadata schema: {error}"))?;

                    sqlx::query(&format!(
                        "CREATE TABLE IF NOT EXISTS {schema_ident}.metadata_audit_entries (
                            id BIGSERIAL PRIMARY KEY,
                            recorded_at_unix_seconds BIGINT NOT NULL,
                            app_id TEXT NOT NULL,
                            trace_id TEXT NOT NULL,
                            request_id TEXT,
                            principal_kind TEXT NOT NULL,
                            principal_id TEXT,
                            kind TEXT NOT NULL
                        )"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!("failed to initialize metadata audit table: {error}"))?;

                    sqlx::query(&format!(
                        "CREATE INDEX IF NOT EXISTS metadata_audit_entries_recent
                            ON {schema_ident}.metadata_audit_entries (recorded_at_unix_seconds DESC, id DESC)"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!("failed to initialize metadata audit index: {error}"))?;

                    Ok(())
                })
            })
            .clone()
    }

    fn qualified_table(&self) -> String {
        format!(
            "{}.{}",
            quote_identifier(&self.schema),
            quote_identifier("metadata_audit_entries")
        )
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn run_blocking<T, F>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn runtime_metadata_backend_labels_the_selected_backend_and_location() {
        let runtime = DataRuntime {
            driver: davenda_config::DatabaseDriver::Postgres,
            connection_secret_ref: None,
            connection_secret: None,
            schema: "public".to_string(),
            migrations_table: "migrations".to_string(),
            pool: davenda_data::ConnectionPoolProfile {
                min_connections: 1,
                max_connections: 4,
                statement_timeout: Duration::from_secs(30),
            },
        };
        let backend = PostgresMetadataAuditStore::open(runtime);

        assert_eq!(backend.location_label(), "public.metadata_audit_entries");
    }
}
