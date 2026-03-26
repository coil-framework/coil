use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use super::*;

#[derive(Debug, Clone)]
pub(super) struct LocalWebhookObservationStore {
    path: PathBuf,
    connection: Arc<Mutex<Connection>>,
}

impl LocalWebhookObservationStore {
    pub(super) fn open(root: PathBuf, namespace: String) -> Self {
        let path = database_path(&root, &namespace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create webhook observation directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open local webhook observation store `{}`: {error}",
                path.display()
            )
        });
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = FULL;
                CREATE TABLE IF NOT EXISTS webhook_observation_entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    recorded_at_unix_seconds INTEGER NOT NULL,
                    app_id TEXT NOT NULL,
                    source TEXT NOT NULL,
                    event TEXT NOT NULL,
                    status TEXT NOT NULL,
                    trace_id TEXT NOT NULL,
                    principal_kind TEXT NOT NULL,
                    principal_id TEXT,
                    detail TEXT
                );
                CREATE INDEX IF NOT EXISTS webhook_observation_entries_recent
                    ON webhook_observation_entries (recorded_at_unix_seconds DESC, id DESC);
                CREATE INDEX IF NOT EXISTS webhook_observation_entries_lookup
                    ON webhook_observation_entries (source, event, status);
                "#,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to initialize local webhook observation store `{}`: {error}",
                    path.display()
                )
            });

        Self {
            path,
            connection: Arc::new(Mutex::new(connection)),
        }
    }

    pub(super) fn location_label(&self) -> String {
        format!("local-sqlite:{}", self.path.display())
    }

    pub(super) fn path(&self) -> &Path {
        self.path.as_path()
    }

    pub(super) fn insert(&self, record: &WebhookObservationEvent) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "webhook observation store is poisoned".to_string())?;
        let tx = connection.transaction().map_err(|error| {
            format!("failed to start local webhook observation transaction: {error}")
        })?;
        tx.execute(
            r#"
            INSERT INTO webhook_observation_entries (
                recorded_at_unix_seconds,
                app_id,
                source,
                event,
                status,
                trace_id,
                principal_kind,
                principal_id,
                detail
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                record.recorded_at_unix_seconds,
                &record.app_id,
                &record.source,
                &record.event,
                record.status.as_str(),
                &record.trace_id,
                &record.principal_kind,
                &record.principal_id,
                &record.detail,
            ],
        )
        .map_err(|error| format!("failed to write local webhook observation entry: {error}"))?;
        tx.commit().map_err(|error| {
            format!("failed to commit local webhook observation entry: {error}")
        })?;
        Ok(())
    }

    pub(super) fn count(&self) -> Result<usize, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "webhook observation store is poisoned".to_string())?;
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM webhook_observation_entries",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                format!("failed to count local webhook observation entries: {error}")
            })?;
        usize::try_from(count)
            .map_err(|_| "local webhook observation entry count overflowed usize".to_string())
    }

    pub(super) fn status_counts(&self) -> Result<WebhookObservationStatusCounts, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "webhook observation store is poisoned".to_string())?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT status, COUNT(*)
                FROM webhook_observation_entries
                GROUP BY status
                "#,
            )
            .map_err(|error| {
                format!("failed to query local webhook observation counts: {error}")
            })?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|error| format!("failed to map local webhook observation counts: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                format!("failed to collect local webhook observation counts: {error}")
            })?;
        decode_status_counts(rows)
    }

    pub(super) fn recent(&self, limit: usize) -> Result<Vec<WebhookObservationEvent>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let connection = self
            .connection
            .lock()
            .map_err(|_| "webhook observation store is poisoned".to_string())?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT
                    id,
                    recorded_at_unix_seconds,
                    app_id,
                    source,
                    event,
                    status,
                    trace_id,
                    principal_kind,
                    principal_id,
                    detail
                FROM webhook_observation_entries
                ORDER BY recorded_at_unix_seconds DESC, id DESC
                LIMIT ?1
                "#,
            )
            .map_err(|error| {
                format!("failed to query local webhook observation entries: {error}")
            })?;
        let mut records = statement
            .query_map(params![limit as i64], |row| {
                let status = row.get::<_, String>(5)?;
                let status = WebhookObservationStatus::from_db_value(&status).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
                    )
                })?;
                Ok(WebhookObservationEvent {
                    id: row.get(0)?,
                    recorded_at_unix_seconds: row.get(1)?,
                    app_id: row.get(2)?,
                    source: row.get(3)?,
                    event: row.get(4)?,
                    status,
                    trace_id: row.get(6)?,
                    principal_kind: row.get(7)?,
                    principal_id: row.get(8)?,
                    detail: row.get(9)?,
                })
            })
            .map_err(|error| format!("failed to map local webhook observation entries: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                format!("failed to collect local webhook observation entries: {error}")
            })?;
        records.reverse();
        Ok(records)
    }
}

fn database_path(root: &Path, namespace: &str) -> PathBuf {
    root.join("wasm")
        .join("webhooks")
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

fn sanitize_namespace(namespace: &str) -> String {
    namespace
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
