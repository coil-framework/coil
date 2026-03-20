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
use crate::identifiers::{JobId, JobQueueName};
#[cfg(not(test))]
use crate::model::{DeadLetterReason, JobInstant};
#[cfg(not(test))]
use crate::runtime::JobSpec;
#[cfg(not(test))]
use sqlx::{Postgres, Row};
#[cfg(not(test))]
use std::env;
#[cfg(not(test))]
use std::future::Future;
#[cfg(not(test))]
use tokio::runtime::Runtime;

#[cfg(not(test))]
pub fn live_shared_runtime(
    runtime: &JobsRuntime,
    namespace: impl Into<String>,
    _root: impl Into<PathBuf>,
) -> Arc<dyn JobsCoordinationRuntime> {
    Arc::new(ProductionPostgresSharedJobsCoordinationRuntime::new(
        runtime.clone(),
        namespace.into(),
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
    fn new(runtime: JobsRuntime, namespace: String) -> Self {
        Self {
            store: ProductionPostgresSharedJobsStore::open(&runtime, namespace),
            runtime,
        }
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
    runtime: Runtime,
    backend: davenda_config::JobBackend,
    namespace: String,
}

#[cfg(not(test))]
impl ProductionPostgresSharedJobsStore {
    fn open(runtime: &JobsRuntime, namespace: String) -> Self {
        let url = jobs_backend_url(runtime.backend);
        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(4)
            .connect_lazy(&url)
            .unwrap_or_else(|error| {
                panic!("failed to open postgres jobs backend `{url}`: {error}")
            });
        let executor = Runtime::new()
            .unwrap_or_else(|error| panic!("failed to create postgres jobs runtime: {error}"));
        Self {
            pool,
            runtime: executor,
            backend: runtime.backend,
            namespace,
        }
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        self.runtime.block_on(future)
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
fn jobs_backend_url(backend: davenda_config::JobBackend) -> String {
    match backend {
        davenda_config::JobBackend::Redis => {
            env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/davenda".to_string())
        }
        davenda_config::JobBackend::Valkey => {
            env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/davenda".to_string())
        }
    }
}
