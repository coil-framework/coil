use super::*;
use crate::backend::JobsBackendState;
use crate::error::JobsModelError;
use bincode::{deserialize, serialize};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::sync::Mutex;

const SHARED_STATE_DIR_ENV: &str = "DAVENDA_SHARED_STATE_DIR";

fn job_backend_slug(backend: davenda_config::JobBackend) -> &'static str {
    match backend {
        davenda_config::JobBackend::Redis => "redis",
        davenda_config::JobBackend::Valkey => "valkey",
    }
}

fn shared_state_root() -> PathBuf {
    std::env::var_os(SHARED_STATE_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("davenda-shared"))
}

fn database_path(runtime: &JobsRuntime, namespace: &str) -> PathBuf {
    shared_state_root()
        .join("jobs")
        .join(job_backend_slug(runtime.backend))
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

#[derive(Debug)]
pub(super) struct SharedJobsStore {
    connection: Mutex<Connection>,
    namespace: String,
}

impl SharedJobsStore {
    pub(super) fn open(runtime: &JobsRuntime, namespace: String) -> Self {
        let path = database_path(runtime, &namespace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create persistent jobs backend directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open persistent jobs backend `{}`: {error}",
                path.display()
            )
        });
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                CREATE TABLE IF NOT EXISTS jobs_state (
                    namespace TEXT PRIMARY KEY,
                    payload BLOB NOT NULL
                );
                "#,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to initialize persistent jobs backend `{}`: {error}",
                    path.display()
                )
            });

        Self {
            connection: Mutex::new(connection),
            namespace,
        }
    }

    pub(super) fn read_snapshot<T>(
        &self,
        op: impl FnOnce(&crate::JobsCoordinatorSnapshot) -> T,
    ) -> Result<T, JobsModelError> {
        let connection = self.connection.lock().expect("jobs backend mutex poisoned");
        let payload = connection
            .query_row(
                "SELECT payload FROM jobs_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read persistent jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let snapshot = match payload {
            Some(payload) => deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize persistent jobs backend state for `{}`: {error}",
                    self.namespace
                )
            }),
            None => crate::JobsCoordinatorSnapshot::default(),
        };

        Ok(op(&snapshot))
    }

    pub(super) fn with_state_mut<T>(
        &self,
        runtime: &JobsRuntime,
        op: impl FnOnce(&mut JobsBackendState) -> Result<T, JobsModelError>,
    ) -> Result<T, JobsModelError> {
        let mut connection = self.connection.lock().expect("jobs backend mutex poisoned");
        let tx = connection.transaction().unwrap_or_else(|error| {
            panic!(
                "failed to start persistent jobs backend transaction for `{}`: {error}",
                self.namespace
            )
        });
        let payload = tx
            .query_row(
                "SELECT payload FROM jobs_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read persistent jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let snapshot = match payload {
            Some(payload) => deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize persistent jobs backend state for `{}`: {error}",
                    self.namespace
                )
            }),
            None => crate::JobsCoordinatorSnapshot::default(),
        };

        let mut state = JobsBackendState {
            runtime: runtime.clone(),
            snapshot,
        };

        let outcome = op(&mut state);
        if outcome.is_ok() {
            let payload = serialize(&state.snapshot).unwrap_or_else(|error| {
                panic!(
                    "failed to serialize persistent jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.execute(
                "INSERT INTO jobs_state (namespace, payload) VALUES (?1, ?2)
                 ON CONFLICT(namespace) DO UPDATE SET payload = excluded.payload",
                params![self.namespace.as_str(), payload],
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to persist jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.commit().unwrap_or_else(|error| {
                panic!(
                    "failed to commit persistent jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });
        }

        outcome
    }
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
