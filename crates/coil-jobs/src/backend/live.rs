use std::path::PathBuf;
#[cfg(not(test))]
use std::time::Duration;

#[cfg(not(test))]
use super::state::JobsBackendState;
use super::*;
#[cfg(not(test))]
use crate::backend::{JobFailureDisposition, JobLease, SchedulerLeadership};
#[cfg(not(test))]
use crate::error::JobsModelError;
#[cfg(not(test))]
use crate::identifiers::{DeadLetterId, JobId, JobQueueName};
#[cfg(not(test))]
use crate::model::{DeadLetterReason, JobInstant};
#[cfg(not(test))]
use crate::runtime::JobSpec;
#[cfg(not(test))]
use sqlx::{Postgres, Row};
#[cfg(not(test))]
use std::future::Future;
#[cfg(not(test))]
use tokio::runtime::{Handle, Runtime};
#[cfg(not(test))]
use tokio::task;

#[cfg(not(test))]
pub fn live_shared_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
    _root: impl Into<PathBuf>,
) -> Result<Arc<dyn JobsCoordinationRuntime>, JobsModelError> {
    Ok(Arc::new(
        ProductionPostgresSharedJobsCoordinationRuntime::new(runtime.clone(), namespace.into())?,
    ))
}

#[cfg(test)]
pub fn live_shared_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
    _root: impl Into<PathBuf>,
) -> Arc<dyn JobsCoordinationRuntime> {
    super::shared::test_only_sqlite_shared_runtime(runtime, namespace.into())
}

#[cfg(not(test))]
#[derive(Debug)]
struct ProductionPostgresSharedJobsCoordinationRuntime {
    runtime: JobsRuntime,
    store: ProductionPostgresSharedJobsStore,
}

#[cfg(not(test))]
impl ProductionPostgresSharedJobsCoordinationRuntime {
    fn new(runtime: JobsRuntime, namespace: String) -> Result<Self, JobsModelError> {
        let store = ProductionPostgresSharedJobsStore::open(&runtime, namespace)?;
        store.read_snapshot(|_| ())?;

        Ok(Self { store, runtime })
    }
}

#[cfg(not(test))]
impl JobsCoordinationRuntime for ProductionPostgresSharedJobsCoordinationRuntime {
    fn snapshot(&self) -> crate::JobsCoordinatorSnapshot {
        self.store
            .read_snapshot(|snapshot| snapshot.clone())
            .expect("postgres jobs backend snapshot read failed")
    }

    fn enqueue(&self, spec: JobSpec, now: JobInstant) -> Result<(), JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.enqueue(spec, now)?;
            Ok(())
        })
    }

    fn retry_dead_letter(
        &self,
        dead_letter_id: &DeadLetterId,
        now: JobInstant,
    ) -> Result<QueuedJobRecord, JobsModelError> {
        self.store.with_state_mut(&self.runtime, |state| {
            state.retry_dead_letter(dead_letter_id, now)
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

#[cfg(not(test))]
#[derive(Debug)]
struct ProductionPostgresSharedJobsStore {
    pool: sqlx::Pool<Postgres>,
    runtime: Option<Runtime>,
    backend: coil_config::JobBackend,
    namespace: String,
}

#[cfg(not(test))]
impl ProductionPostgresSharedJobsStore {
    fn open(runtime: &JobsRuntime, namespace: String) -> Result<Self, JobsModelError> {
        let url = jobs_backend_url(runtime.backend, std::env::var("DATABASE_URL").ok())?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(4)
            .connect_lazy(&url)
            .map_err(
                |error| JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: runtime.backend,
                    namespace: format!("failed to open postgres jobs backend `{url}`: {error}"),
                },
            )?;
        let executor = Runtime::new().map_err(|error| {
            JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                backend: runtime.backend,
                namespace: format!("failed to create postgres jobs runtime: {error}"),
            }
        })?;
        Ok(Self {
            pool,
            runtime: Some(executor),
            backend: runtime.backend,
            namespace,
        })
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        match Handle::try_current() {
            Ok(handle) => task::block_in_place(|| handle.block_on(future)),
            Err(_) => self
                .runtime
                .as_ref()
                .expect("jobs runtime missing during live operation")
                .block_on(future),
        }
    }

    async fn ensure_table(&self) -> Result<(), JobsModelError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS jobs_state (
                namespace TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(
            |error| JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                backend: self.backend,
                namespace: error.to_string(),
            },
        )?;
        Ok(())
    }

    fn read_snapshot<T>(
        &self,
        op: impl FnOnce(&crate::JobsCoordinatorSnapshot) -> T,
    ) -> Result<T, JobsModelError> {
        self.block_on(async {
            self.ensure_table().await?;
            let payload = sqlx::query("SELECT payload FROM jobs_state WHERE namespace = $1")
                .bind(&self.namespace)
                .fetch_optional(&self.pool)
                .await
                .map_err(
                    |error| JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                        backend: self.backend,
                        namespace: error.to_string(),
                    },
                )?
                .map(|row| row.get::<String, _>("payload"));

            let snapshot = match payload {
                Some(payload) => serde_json::from_str(&payload).map_err(|error| {
                    JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                        backend: self.backend,
                        namespace: error.to_string(),
                    }
                })?,
                None => crate::JobsCoordinatorSnapshot::default(),
            };

            Ok(op(&snapshot))
        })
    }

    fn with_state_mut<T>(
        &self,
        runtime: &JobsRuntime,
        op: impl FnOnce(&mut JobsBackendState) -> Result<T, JobsModelError>,
    ) -> Result<T, JobsModelError> {
        self.block_on(async {
            self.ensure_table().await?;
            let mut tx = self.pool.begin().await.map_err(|error| {
                JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: runtime.backend,
                    namespace: error.to_string(),
                }
            })?;

            sqlx::query(
                "INSERT INTO jobs_state (namespace, payload) VALUES ($1, $2) ON CONFLICT(namespace) DO NOTHING",
            )
            .bind(&self.namespace)
            .bind(
                serde_json::to_string(&crate::JobsCoordinatorSnapshot::default()).unwrap(),
            )
            .execute(&mut *tx)
            .await
            .map_err(|error| JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                backend: runtime.backend,
                namespace: error.to_string(),
            })?;

            let payload = sqlx::query("SELECT payload FROM jobs_state WHERE namespace = $1 FOR UPDATE")
                .bind(&self.namespace)
                .fetch_one(&mut *tx)
                .await
                .map_err(|error| JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: runtime.backend,
                    namespace: error.to_string(),
                })?
                .get::<String, _>("payload");

            let snapshot = serde_json::from_str(&payload).map_err(|error| {
                JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: self.backend,
                    namespace: error.to_string(),
                }
            })?;

            let mut state = JobsBackendState {
                runtime: runtime.clone(),
                snapshot,
            };
            let outcome = op(&mut state)?;
            let payload = serde_json::to_string(&state.snapshot).map_err(|error| {
                JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: state.runtime.backend,
                    namespace: error.to_string(),
                }
            })?;

            sqlx::query("UPDATE jobs_state SET payload = $2 WHERE namespace = $1")
                .bind(&self.namespace)
                .bind(payload)
                .execute(&mut *tx)
                .await
                .map_err(|error| {
                    JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                        backend: state.runtime.backend,
                        namespace: error.to_string(),
                    }
                })?;
            tx.commit().await.map_err(|error| {
                JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: state.runtime.backend,
                    namespace: error.to_string(),
                }
            })?;

            Ok(outcome)
        })
    }
}

#[cfg(not(test))]
impl Drop for ProductionPostgresSharedJobsStore {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            std::thread::spawn(move || drop(runtime))
                .join()
                .expect("jobs runtime drop thread panicked");
        }
    }
}

fn jobs_backend_url(
    backend: coil_config::JobBackend,
    database_url: Option<String>,
) -> Result<String, crate::error::JobsModelError> {
    match backend {
        coil_config::JobBackend::Redis | coil_config::JobBackend::Valkey => database_url
            .ok_or_else(|| {
                crate::error::JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend,
                    namespace: "missing environment variable DATABASE_URL".to_string(),
                }
            }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::JobsModelError;

    #[test]
    fn live_jobs_backend_requires_explicit_database_url() {
        let error = jobs_backend_url(coil_config::JobBackend::Redis, None).unwrap_err();

        assert_eq!(
            error,
            JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                backend: coil_config::JobBackend::Redis,
                namespace: "missing environment variable DATABASE_URL".to_string(),
            }
        );
    }
}
