use super::host::BrowserHostBuildError;
use super::session::{
    BrowserInstant, BrowserSessionRecord, DistributedSessionStoreRuntime, SessionStoreBackendKind,
};
use super::RuntimeBrowserError;
use std::env;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use sqlx::{Postgres, Row as PgRow};

pub(crate) fn live_shared_runtime(
    kind: SessionStoreBackendKind,
    namespace: impl Into<String>,
    _root: impl Into<PathBuf>,
) -> Result<Arc<dyn DistributedSessionStoreRuntime>, BrowserHostBuildError> {
    let namespace = namespace.into();
    Ok(Arc::new(ProductionPostgresSharedSessionStoreRuntime::new(
        kind,
        namespace,
    )?))
}

#[derive(Debug)]
struct ProductionPostgresSharedSessionStoreRuntime {
    store: ProductionPostgresSharedSessionStore,
}

impl ProductionPostgresSharedSessionStoreRuntime {
    fn new(kind: SessionStoreBackendKind, namespace: String) -> Result<Self, BrowserHostBuildError> {
        Ok(Self {
            store: ProductionPostgresSharedSessionStore::open(kind, namespace)?,
        })
    }
}

impl DistributedSessionStoreRuntime for ProductionPostgresSharedSessionStoreRuntime {
    fn issue(&self, record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError> {
        self.store.with_state_mut(|state| {
            state.issue(record);
            Ok(())
        })
    }

    fn session(
        &self,
        session_id: &str,
    ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
        self.store.read_state(|state| state.session(session_id))
    }

    fn delete(&self, session_id: &str) -> Result<(), RuntimeBrowserError> {
        self.store.with_state_mut(|state| {
            state.sessions.remove(session_id);
            Ok(())
        })
    }

    fn revoke(&self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        self.store.with_state_mut(|state| state.revoke(session_id, now))
    }

    fn touch_active_session(
        &self,
        session_id: &str,
        idle_timeout: std::time::Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        self.store
            .with_state_mut(|state| state.touch_active_session(session_id, idle_timeout, now))
    }

    fn is_shared_backend(&self) -> bool {
        true
    }

    fn supports_live_shared_state(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct ProductionPostgresSharedSessionStore {
    pool: sqlx::Pool<Postgres>,
    runtime: Runtime,
    kind: SessionStoreBackendKind,
    namespace: String,
}

impl ProductionPostgresSharedSessionStore {
    fn open(
        kind: SessionStoreBackendKind,
        namespace: String,
    ) -> Result<Self, BrowserHostBuildError> {
        let url = session_backend_url();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .min_connections(1)
            .max_connections(4)
            .connect_lazy(&url)
            .map_err(|reason| BrowserHostBuildError::LiveSharedSessionStoreInitializationFailed {
                kind,
                scope: namespace.clone(),
                path: url.clone(),
                reason: reason.to_string(),
            })?;
        let runtime = Runtime::new().map_err(|reason| {
            BrowserHostBuildError::LiveSharedSessionStoreInitializationFailed {
                kind,
                scope: namespace.clone(),
                path: url.clone(),
                reason: reason.to_string(),
            }
        })?;

        Ok(Self {
            pool,
            runtime,
            kind,
            namespace,
        })
    }

    fn block_on<T>(&self, future: impl Future<Output = T>) -> T {
        self.runtime.block_on(future)
    }

    async fn ensure_table(pool: &sqlx::Pool<Postgres>) -> Result<(), RuntimeBrowserError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_state (
                namespace TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .map_err(|error| RuntimeBrowserError::LiveSharedSessionStoreFailure {
            kind: SessionStoreBackendKind::Database,
            scope: "session-state".to_string(),
            reason: error.to_string(),
        })?;
        Ok(())
    }

    fn read_state<T>(
        &self,
        op: impl FnOnce(&SessionStoreSnapshot) -> T,
    ) -> Result<T, RuntimeBrowserError> {
        self.block_on(async {
            Self::ensure_table(&self.pool).await?;
            let payload = sqlx::query("SELECT payload FROM session_state WHERE namespace = $1")
                .bind(&self.namespace)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| RuntimeBrowserError::LiveSharedSessionStoreFailure {
                    kind: self.kind,
                    scope: self.namespace.clone(),
                    reason: error.to_string(),
                })?
                .map(|row| row.get::<String, _>("payload"));

            let state = match payload {
                Some(payload) => serde_json::from_str(&payload).map_err(|error| {
                    RuntimeBrowserError::LiveSharedSessionStoreFailure {
                        kind: self.kind,
                        scope: self.namespace.clone(),
                        reason: error.to_string(),
                    }
                })?,
                None => SessionStoreSnapshot::default(),
            };

            Ok(op(&state))
        })
    }

    fn with_state_mut<T>(
        &self,
        op: impl FnOnce(&mut SessionStoreSnapshot) -> Result<T, RuntimeBrowserError>,
    ) -> Result<T, RuntimeBrowserError> {
        self.block_on(async {
            Self::ensure_table(&self.pool).await?;
            let mut tx = self.pool.begin().await.map_err(|error| {
                RuntimeBrowserError::LiveSharedSessionStoreFailure {
                    kind: self.kind,
                    scope: self.namespace.clone(),
                    reason: error.to_string(),
                }
            })?;

            sqlx::query(
                "INSERT INTO session_state (namespace, payload) VALUES ($1, $2) ON CONFLICT(namespace) DO NOTHING",
            )
            .bind(&self.namespace)
            .bind(serde_json::to_string(&SessionStoreSnapshot::default()).unwrap())
            .execute(&mut *tx)
            .await
            .map_err(|error| RuntimeBrowserError::LiveSharedSessionStoreFailure {
                kind: self.kind,
                scope: self.namespace.clone(),
                reason: error.to_string(),
            })?;

            let payload = sqlx::query(
                "SELECT payload FROM session_state WHERE namespace = $1 FOR UPDATE",
            )
            .bind(&self.namespace)
            .fetch_one(&mut *tx)
            .await
            .map_err(|error| RuntimeBrowserError::LiveSharedSessionStoreFailure {
                kind: self.kind,
                scope: self.namespace.clone(),
                reason: error.to_string(),
            })?
            .get::<String, _>("payload");

            let mut state = serde_json::from_str(&payload).map_err(|error| {
                RuntimeBrowserError::LiveSharedSessionStoreFailure {
                    kind: self.kind,
                    scope: self.namespace.clone(),
                    reason: error.to_string(),
                }
            })?;

            let outcome = op(&mut state);
            let should_persist = matches!(
                outcome.as_ref(),
                Ok(_) | Err(RuntimeBrowserError::ExpiredSession { .. })
            );
            if should_persist {
                let payload = serde_json::to_string(&state).map_err(|error| {
                    RuntimeBrowserError::LiveSharedSessionStoreFailure {
                        kind: self.kind,
                        scope: self.namespace.clone(),
                        reason: error.to_string(),
                    }
                })?;
                sqlx::query("UPDATE session_state SET payload = $2 WHERE namespace = $1")
                    .bind(&self.namespace)
                    .bind(payload)
                    .execute(&mut *tx)
                    .await
                    .map_err(|error| RuntimeBrowserError::LiveSharedSessionStoreFailure {
                        kind: self.kind,
                        scope: self.namespace.clone(),
                        reason: error.to_string(),
                    })?;
                tx.commit().await.map_err(|error| {
                    RuntimeBrowserError::LiveSharedSessionStoreFailure {
                        kind: self.kind,
                        scope: self.namespace.clone(),
                        reason: error.to_string(),
                    }
                })?;
            }

            outcome
        })
    }
}

fn session_backend_url() -> String {
    env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/davenda".to_string())
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct SessionStoreSnapshot {
    sessions: std::collections::BTreeMap<String, BrowserSessionRecord>,
}

impl SessionStoreSnapshot {
    fn issue(&mut self, record: BrowserSessionRecord) {
        self.sessions.insert(record.session_id.clone(), record);
    }

    fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        self.sessions.get(session_id).cloned()
    }

    fn revoke(&mut self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        let existing = self.sessions.get_mut(session_id).ok_or_else(|| {
            RuntimeBrowserError::UnknownSession {
                session_id: session_id.to_string(),
            }
        })?;
        existing.revoked_at = Some(now);
        Ok(())
    }

    fn touch_active_session(
        &mut self,
        session_id: &str,
        idle_timeout: std::time::Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        let record = self.sessions.get_mut(session_id).ok_or_else(|| {
            RuntimeBrowserError::UnknownSession {
                session_id: session_id.to_string(),
            }
        })?;

        match record.status_at(now) {
            super::BrowserSessionStatus::Active => {
                record.last_seen_at = now;
                record.idle_expires_at = now.saturating_add(idle_timeout);
                Ok(record.principal_id.clone())
            }
            super::BrowserSessionStatus::IdleExpired
            | super::BrowserSessionStatus::AbsoluteExpired => {
                self.sessions.remove(session_id);
                Err(RuntimeBrowserError::ExpiredSession {
                    session_id: session_id.to_string(),
                })
            }
            super::BrowserSessionStatus::Revoked => Err(RuntimeBrowserError::RevokedSession {
                session_id: session_id.to_string(),
            }),
        }
    }
}
