use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, params};

use super::*;

#[derive(Debug, Clone)]
pub(super) struct LocalMetadataAuditStore {
    path: PathBuf,
    connection: Arc<Mutex<Connection>>,
}

impl LocalMetadataAuditStore {
    pub(super) fn open(root: PathBuf, namespace: String) -> Self {
        let path = database_path(&root, &namespace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create metadata audit directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open local metadata audit store `{}`: {error}",
                path.display()
            )
        });
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = FULL;
                CREATE TABLE IF NOT EXISTS metadata_audit_entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    recorded_at_unix_seconds INTEGER NOT NULL,
                    app_id TEXT NOT NULL,
                    trace_id TEXT NOT NULL,
                    request_id TEXT,
                    principal_kind TEXT NOT NULL,
                    principal_id TEXT,
                    kind TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS metadata_audit_entries_recent
                    ON metadata_audit_entries (recorded_at_unix_seconds DESC, id DESC);
                "#,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to initialize local metadata audit store `{}`: {error}",
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

    pub(super) fn insert(&self, record: &MetadataAuditRecord) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "metadata audit store is poisoned".to_string())?;
        let tx = connection.transaction().map_err(|error| {
            format!("failed to start local metadata audit transaction: {error}")
        })?;
        tx.execute(
            r#"
            INSERT INTO metadata_audit_entries (
                recorded_at_unix_seconds,
                app_id,
                trace_id,
                request_id,
                principal_kind,
                principal_id,
                kind
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                record.recorded_at_unix_seconds,
                record.app_id,
                record.trace_id,
                record.request_id,
                record.principal_kind,
                record.principal_id,
                record.kind,
            ],
        )
        .map_err(|error| format!("failed to write local metadata audit entry: {error}"))?;
        tx.commit()
            .map_err(|error| format!("failed to commit local metadata audit entry: {error}"))?;
        Ok(())
    }

    pub(super) fn count(&self) -> Result<usize, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "metadata audit store is poisoned".to_string())?;
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM metadata_audit_entries", [], |row| {
                row.get(0)
            })
            .map_err(|error| format!("failed to count local metadata audit entries: {error}"))?;
        usize::try_from(count)
            .map_err(|_| "local metadata audit entry count overflowed usize".to_string())
    }

    pub(super) fn recent(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let connection = self
            .connection
            .lock()
            .map_err(|_| "metadata audit store is poisoned".to_string())?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT
                    id,
                    recorded_at_unix_seconds,
                    app_id,
                    trace_id,
                    request_id,
                    principal_kind,
                    principal_id,
                    kind
                FROM metadata_audit_entries
                ORDER BY recorded_at_unix_seconds DESC, id DESC
                LIMIT ?1
                "#,
            )
            .map_err(|error| format!("failed to query local metadata audit entries: {error}"))?;

        let mut records = statement
            .query_map(params![limit as i64], |row| {
                Ok(MetadataAuditRecord {
                    id: row.get(0)?,
                    recorded_at_unix_seconds: row.get(1)?,
                    app_id: row.get(2)?,
                    trace_id: row.get(3)?,
                    request_id: row.get(4)?,
                    principal_kind: row.get(5)?,
                    principal_id: row.get(6)?,
                    kind: row.get(7)?,
                })
            })
            .map_err(|error| format!("failed to map local metadata audit entries: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to collect local metadata audit entries: {error}"))?;
        records.reverse();
        Ok(records)
    }
}

fn database_path(root: &Path, namespace: &str) -> PathBuf {
    root.join("wasm")
        .join("metadata")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    use davenda_wasm::MetadataGrant;

    fn shared_state_root(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("davenda-metadata-{}-{}", std::process::id(), label));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn local_metadata_backend_persists_and_queries_audit_records() {
        let root = shared_state_root("persistence");
        let backend = LocalMetadataAuditStore::open(root.clone(), "audit-suite".to_string());

        backend
            .insert(&MetadataAuditRecord {
                id: 0,
                recorded_at_unix_seconds: 1,
                kind: MetadataGrant::JsonLd.to_string(),
                app_id: "audit-app".to_string(),
                trace_id: "trace-1".to_string(),
                request_id: Some("req-1".to_string()),
                principal_kind: "user".to_string(),
                principal_id: Some("alice".to_string()),
            })
            .unwrap();

        backend
            .insert(&MetadataAuditRecord {
                id: 0,
                recorded_at_unix_seconds: 2,
                kind: MetadataGrant::SeoHead.to_string(),
                app_id: "audit-app".to_string(),
                trace_id: "trace-2".to_string(),
                request_id: Some("req-2".to_string()),
                principal_kind: "user".to_string(),
                principal_id: Some("bob".to_string()),
            })
            .unwrap();

        assert_eq!(backend.count().unwrap(), 2);
        let records = backend.recent(10).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].trace_id, "trace-1");
        assert_eq!(records[1].trace_id, "trace-2");

        let last_only = backend.recent(1).unwrap();
        assert_eq!(last_only.len(), 1);
        assert_eq!(last_only[0].trace_id, "trace-2");
    }

    #[test]
    fn local_metadata_backend_labels_the_selected_backend_and_location() {
        let root = shared_state_root("labels");
        let backend = LocalMetadataAuditStore::open(root.clone(), "audit-suite".to_string());

        assert!(backend.location_label().starts_with("local-sqlite:"));
        assert!(backend.path().starts_with(&root));
    }

    #[test]
    fn local_metadata_backend_uses_durable_write_pragmas() {
        let root = shared_state_root("pragmas");
        let backend = LocalMetadataAuditStore::open(root, "audit-suite".to_string());

        let connection = backend.connection.lock().expect("connection mutex should not be poisoned");
        let synchronous: i64 = connection
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .expect("synchronous pragma should be queryable");
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal_mode pragma should be queryable");

        assert_eq!(synchronous, 2, "FULL synchronous mode should be enabled for local audit durability");
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    }
}
