use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use super::state::JobsBackendState;
use super::*;
use crate::backend::{JobFailureDisposition, JobLease, SchedulerLeadership};
use crate::error::JobsModelError;
use crate::identifiers::{JobId, JobQueueName};
use crate::model::{DeadLetterReason, JobInstant};
use crate::runtime::JobSpec;
use rusqlite::{Connection, OptionalExtension, params};

pub fn live_shared_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
    root: impl Into<PathBuf>,
) -> Arc<dyn JobsCoordinationRuntime> {
    Arc::new(LiveSharedJobsCoordinationRuntime::new(
        runtime.clone(),
        namespace.into(),
        root.into(),
    ))
}

#[derive(Debug)]
pub(super) struct LiveSharedJobsCoordinationRuntime {
    runtime: JobsRuntime,
    store: LiveSharedJobsStore,
}

impl LiveSharedJobsCoordinationRuntime {
    fn new(runtime: JobsRuntime, namespace: String, root: PathBuf) -> Self {
        Self {
            store: LiveSharedJobsStore::open(&runtime, namespace, root),
            runtime,
        }
    }
}

impl JobsCoordinationRuntime for LiveSharedJobsCoordinationRuntime {
    fn snapshot(&self) -> crate::JobsCoordinatorSnapshot {
        self.store
            .read_snapshot(|snapshot| snapshot.clone())
            .expect("live jobs backend snapshot read failed")
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

    fn supports_live_shared_state(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct LiveSharedJobsStore {
    connection: Mutex<Connection>,
    namespace: String,
}

impl LiveSharedJobsStore {
    fn open(runtime: &JobsRuntime, namespace: String, root: PathBuf) -> Self {
        let path = live_database_path(runtime, &namespace, root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create live jobs backend directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open live jobs backend `{}`: {error}",
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
                    "failed to initialize live jobs backend `{}`: {error}",
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
                    "failed to read live jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let snapshot = match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize live jobs backend state for `{}`: {error}",
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
                "failed to start live jobs backend transaction for `{}`: {error}",
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
                    "failed to read live jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let snapshot = match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize live jobs backend state for `{}`: {error}",
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
            let payload = bincode::serialize(&state.snapshot).unwrap_or_else(|error| {
                panic!(
                    "failed to serialize live jobs backend state for `{}`: {error}",
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
                    "failed to persist live jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.commit().unwrap_or_else(|error| {
                panic!(
                    "failed to commit live jobs backend state for `{}`: {error}",
                    self.namespace
                )
            });
        }

        outcome
    }
}

fn live_database_path(runtime: &JobsRuntime, namespace: &str, root: PathBuf) -> PathBuf {
    root.join("jobs")
        .join(job_backend_slug(runtime.backend))
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

fn job_backend_slug(backend: davenda_config::JobBackend) -> &'static str {
    match backend {
        davenda_config::JobBackend::Redis => "redis",
        davenda_config::JobBackend::Valkey => "valkey",
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
