use std::sync::OnceLock;

use davenda_data::{DataRuntime, PostgresDataClient};
use sqlx::Row;

use super::*;

#[derive(Debug, Clone)]
pub(super) struct SharedWebhookObservationStore {
    runtime: DataRuntime,
    client: OnceLock<Result<PostgresDataClient, String>>,
    schema: String,
    initialized: OnceLock<Result<(), String>>,
}

impl SharedWebhookObservationStore {
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
        format!(
            "shared-postgres:{}.webhook_observation_entries",
            self.schema
        )
    }

    pub(super) fn insert(&self, record: &WebhookObservationEvent) -> Result<(), String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        let record = record.clone();
        run_blocking(async move {
            sqlx::query(&format!(
                "INSERT INTO {} (recorded_at_unix_seconds, app_id, source, event, status, trace_id, principal_kind, principal_id, detail) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                table
            ))
            .bind(record.recorded_at_unix_seconds)
            .bind(&record.app_id)
            .bind(&record.source)
            .bind(&record.event)
            .bind(record.status.as_str())
            .bind(&record.trace_id)
            .bind(&record.principal_kind)
            .bind(&record.principal_id)
            .bind(&record.detail)
            .execute(&client.pool)
            .await
            .map_err(|error| format!("failed to write shared webhook observation entry: {error}"))?;
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
                .map_err(|error| {
                    format!("failed to count shared webhook observation entries: {error}")
                })?;
            usize::try_from(count)
                .map_err(|_| "shared webhook observation entry count overflowed usize".to_string())
        })
    }

    pub(super) fn status_counts(&self) -> Result<WebhookObservationStatusCounts, String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            let rows = sqlx::query(&format!(
                "SELECT status, COUNT(*) FROM {} GROUP BY status",
                table
            ))
            .fetch_all(&client.pool)
            .await
            .map_err(|error| {
                format!("failed to query shared webhook observation counts: {error}")
            })?;
            let decoded = rows
                .into_iter()
                .map(|row| {
                    let status: String = row.try_get(0).map_err(|error| {
                        format!("failed to decode shared webhook observation status: {error}")
                    })?;
                    let count: i64 = row.try_get(1).map_err(|error| {
                        format!("failed to decode shared webhook observation count: {error}")
                    })?;
                    Ok((status, count))
                })
                .collect::<Result<Vec<_>, String>>()?;
            decode_status_counts(decoded)
        })
    }

    pub(super) fn recent(&self, limit: usize) -> Result<Vec<WebhookObservationEvent>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            let rows = sqlx::query(&format!(
                "SELECT id, recorded_at_unix_seconds, app_id, source, event, status, trace_id, principal_kind, principal_id, detail FROM {} ORDER BY recorded_at_unix_seconds DESC, id DESC LIMIT $1",
                table
            ))
            .bind(limit as i64)
            .fetch_all(&client.pool)
            .await
            .map_err(|error| format!("failed to query shared webhook observation entries: {error}"))?;
            let mut records = rows
                .into_iter()
                .map(|row| {
                    let status: String = row.try_get(5).map_err(|error| {
                        format!("failed to decode shared webhook observation status: {error}")
                    })?;
                    Ok(WebhookObservationEvent {
                        id: row.try_get(0).map_err(|error| {
                            format!("failed to decode shared webhook observation id: {error}")
                        })?,
                        recorded_at_unix_seconds: row.try_get(1).map_err(|error| {
                            format!(
                                "failed to decode shared webhook observation timestamp: {error}"
                            )
                        })?,
                        app_id: row.try_get(2).map_err(|error| {
                            format!("failed to decode shared webhook observation app id: {error}")
                        })?,
                        source: row.try_get(3).map_err(|error| {
                            format!("failed to decode shared webhook observation source: {error}")
                        })?,
                        event: row.try_get(4).map_err(|error| {
                            format!("failed to decode shared webhook observation event: {error}")
                        })?,
                        status: WebhookObservationStatus::from_db_value(&status)?,
                        trace_id: row.try_get(6).map_err(|error| {
                            format!("failed to decode shared webhook observation trace id: {error}")
                        })?,
                        principal_kind: row.try_get(7).map_err(|error| {
                            format!(
                                "failed to decode shared webhook observation principal kind: {error}"
                            )
                        })?,
                        principal_id: row.try_get(8).map_err(|error| {
                            format!(
                                "failed to decode shared webhook observation principal id: {error}"
                            )
                        })?,
                        detail: row.try_get(9).map_err(|error| {
                            format!("failed to decode shared webhook observation detail: {error}")
                        })?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            records.reverse();
            Ok(records)
        })
    }

    pub(super) fn claim_delivery(
        &self,
        app_id: &str,
        route_name: &str,
        source: &str,
        delivery_id: &str,
        request_id: &str,
        recorded_at_unix_seconds: i64,
    ) -> Result<bool, String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_delivery_table();
        let app_id = app_id.to_string();
        let route_name = route_name.to_string();
        let source = source.to_string();
        let delivery_id = delivery_id.to_string();
        let request_id = request_id.to_string();
        run_blocking(async move {
            let rows = sqlx::query(&format!(
                "INSERT INTO {} (app_id, route_name, source, delivery_id, first_seen_request_id, first_seen_at_unix_seconds) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (app_id, source, delivery_id) DO NOTHING",
                table
            ))
            .bind(&app_id)
            .bind(&route_name)
            .bind(&source)
            .bind(&delivery_id)
            .bind(&request_id)
            .bind(recorded_at_unix_seconds)
            .execute(&client.pool)
            .await
            .map_err(|error| format!("failed to persist shared verified webhook delivery: {error}"))?
            .rows_affected();
            Ok(rows > 0)
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
                        .map_err(|error| {
                            format!(
                                "failed to initialize shared webhook observation schema: {error}"
                            )
                        })?;
                    sqlx::query(&format!(
                        "CREATE TABLE IF NOT EXISTS {schema_ident}.webhook_observation_entries (
                            id BIGSERIAL PRIMARY KEY,
                            recorded_at_unix_seconds BIGINT NOT NULL,
                            app_id TEXT NOT NULL,
                            source TEXT NOT NULL,
                            event TEXT NOT NULL,
                            status TEXT NOT NULL,
                            trace_id TEXT NOT NULL,
                            principal_kind TEXT NOT NULL,
                            principal_id TEXT,
                            detail TEXT
                        )"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!(
                        "failed to initialize shared webhook observation table: {error}"
                    ))?;
                    sqlx::query(&format!(
                        "CREATE INDEX IF NOT EXISTS webhook_observation_entries_recent
                            ON {schema_ident}.webhook_observation_entries (recorded_at_unix_seconds DESC, id DESC)"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!(
                        "failed to initialize shared webhook observation recent index: {error}"
                    ))?;
                    sqlx::query(&format!(
                        "CREATE INDEX IF NOT EXISTS webhook_observation_entries_lookup
                            ON {schema_ident}.webhook_observation_entries (source, event, status)"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!(
                        "failed to initialize shared webhook observation lookup index: {error}"
                    ))?;
                    sqlx::query(&format!(
                        "CREATE TABLE IF NOT EXISTS {schema_ident}.verified_webhook_deliveries (
                            app_id TEXT NOT NULL,
                            route_name TEXT NOT NULL,
                            source TEXT NOT NULL,
                            delivery_id TEXT NOT NULL,
                            first_seen_request_id TEXT NOT NULL,
                            first_seen_at_unix_seconds BIGINT NOT NULL,
                            PRIMARY KEY (app_id, source, delivery_id)
                        )"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!(
                        "failed to initialize shared verified webhook delivery table: {error}"
                    ))?;
                    Ok(())
                })
            })
            .clone()
    }

    fn qualified_table(&self) -> String {
        format!(
            "{}.{}",
            quote_identifier(&self.schema),
            quote_identifier("webhook_observation_entries")
        )
    }

    fn qualified_delivery_table(&self) -> String {
        format!(
            "{}.{}",
            quote_identifier(&self.schema),
            quote_identifier("verified_webhook_deliveries")
        )
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn run_blocking<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(future))
            }
            tokio::runtime::RuntimeFlavor::CurrentThread => run_future_on_dedicated_runtime(future),
            _ => run_future_on_dedicated_runtime(future),
        },
        Err(_) => run_future_on_ephemeral_runtime(future),
    }
}

fn run_future_on_dedicated_runtime<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(future)
    })
    .join()
    .map_err(|_| "shared webhook worker thread panicked".to_string())?
}

fn run_future_on_ephemeral_runtime<T, F>(future: F) -> Result<T, String>
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(future)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_webhook_run_blocking_works_inside_current_thread_runtime() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let value = runtime.block_on(async { run_blocking(async { Ok::<_, String>(11usize) }) });

        assert_eq!(value.unwrap(), 11);
    }
}
