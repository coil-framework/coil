use super::{JobsBackendState, JobsCoordinationRuntime, JobsRuntime};
use crate::backend::{JobFailureDisposition, JobLease, SchedulerLeadership};
use crate::error::JobsModelError;
use crate::identifiers::{JobId, JobQueueName};
use crate::model::{DeadLetterReason, JobInstant};
use crate::runtime::JobSpec;
use bincode::{deserialize, serialize};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SHARED_STATE_DIR_ENV: &str = "DAVENDA_SHARED_STATE_DIR";

pub(crate) fn persistent_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
) -> Arc<dyn JobsCoordinationRuntime> {
    Arc::new(PersistentJobsCoordinationRuntime::new(
        runtime.clone(),
        namespace.into(),
    ))
}

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

fn database_path(runtime: &JobsRuntime) -> PathBuf {
    shared_state_root()
        .join("jobs")
        .join(format!("{}.sqlite3", job_backend_slug(runtime.backend)))
}

#[derive(Debug)]
struct PersistentJobsCoordinationRuntime {
    runtime: JobsRuntime,
    store: SharedJobsStore,
}

impl PersistentJobsCoordinationRuntime {
    fn new(runtime: JobsRuntime, namespace: String) -> Self {
        Self {
            store: SharedJobsStore::open(&runtime, namespace),
            runtime,
        }
    }
}

impl JobsCoordinationRuntime for PersistentJobsCoordinationRuntime {
    fn snapshot(&self) -> crate::JobsCoordinatorSnapshot {
        self.store
            .read_snapshot(|snapshot| snapshot.clone())
            .expect("persistent jobs backend snapshot read failed")
    }

    fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.enqueue(spec, now)?;
            Ok(())
        })
    }

    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: JobInstant,
        lease_ttl: Duration,
    ) -> Result<SchedulerLeadership, JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.acquire_scheduler_leadership(node_id, now, lease_ttl)
        })
    }

    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: JobInstant,
    ) -> Result<Vec<JobId>, JobsModelError> {
        self.store
            .with_state_mut(&self.runtime, |state| state.promote_due_jobs(node_id, now))
    }

    fn lease_ready_jobs(
        &self,
        queue: &JobQueueName,
        worker_id: String,
        now: JobInstant,
        lease_ttl: Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)
        })
    }

    fn acknowledge_completed(
        &self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.acknowledge_completed(lease, now)
        })?;
        Ok(())
    }

    fn acknowledge_failed(
        &self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: String,
    ) -> Result<JobFailureDisposition, JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.acknowledge_failed(lease, now, reason, error_message)
        })
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct SharedJobsStore {
    connection: Mutex<Connection>,
    namespace: String,
}

impl SharedJobsStore {
    fn open(runtime: &JobsRuntime, namespace: String) -> Self {
        let path = database_path(runtime);
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

    fn read_snapshot<T>(
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

    fn with_state_mut<T>(
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
