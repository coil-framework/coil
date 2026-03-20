#![cfg_attr(test, allow(dead_code))]

use super::host::BrowserHostBuildError;
use super::session::{
    BrowserInstant, BrowserSessionRecord, DistributedSessionStoreRuntime, SessionStoreBackendKind,
};
use super::RuntimeBrowserError;
use rusqlite::{Connection, OptionalExtension, Row, params};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) fn live_shared_runtime(
    kind: SessionStoreBackendKind,
    namespace: impl Into<String>,
    root: impl Into<PathBuf>,
) -> Result<Arc<dyn DistributedSessionStoreRuntime>, BrowserHostBuildError> {
    let namespace = namespace.into();
    let path = live_database_path(kind, &namespace, root.into());
    let runtime = LiveSharedSessionStoreRuntime::new(kind, namespace, path)?;
    Ok(Arc::new(runtime))
}

#[derive(Debug)]
struct LiveSharedSessionStoreRuntime {
    store: LiveSharedSessionStore,
}

impl LiveSharedSessionStoreRuntime {
    fn new(
        kind: SessionStoreBackendKind,
        namespace: String,
        path: PathBuf,
    ) -> Result<Self, BrowserHostBuildError> {
        Ok(Self {
            store: LiveSharedSessionStore::open(kind, namespace, path)?,
        })
    }
}

impl DistributedSessionStoreRuntime for LiveSharedSessionStoreRuntime {
    fn issue(&self, record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError> {
        self.store
            .with_state_mut(|state| {
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
        self.store
            .with_state_mut(|state| {
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

#[derive(Debug)]
struct LiveSharedSessionStore {
    connection: Mutex<Connection>,
    kind: SessionStoreBackendKind,
    namespace: String,
}

impl LiveSharedSessionStore {
    fn open(
        kind: SessionStoreBackendKind,
        namespace: String,
        path: PathBuf,
    ) -> Result<Self, BrowserHostBuildError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|reason| {
                BrowserHostBuildError::LiveSharedSessionStoreInitializationFailed {
                    kind,
                    scope: namespace.clone(),
                    path: path.display().to_string(),
                    reason: reason.to_string(),
                }
            })?;
        }

        let connection = Connection::open(&path).map_err(|reason| {
            BrowserHostBuildError::LiveSharedSessionStoreInitializationFailed {
                kind,
                scope: namespace.clone(),
                path: path.display().to_string(),
                reason: reason.to_string(),
            }
        })?;
        connection.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            CREATE TABLE IF NOT EXISTS session_state (
                namespace TEXT PRIMARY KEY,
                payload TEXT NOT NULL
            );
            "#,
        )
        .map_err(|reason| BrowserHostBuildError::LiveSharedSessionStoreInitializationFailed {
            kind,
            scope: namespace.clone(),
            path: path.display().to_string(),
            reason: reason.to_string(),
        })?;

        Ok(Self {
            connection: Mutex::new(connection),
            kind,
            namespace,
        })
    }

    fn error(&self, reason: impl Into<String>) -> RuntimeBrowserError {
        RuntimeBrowserError::LiveSharedSessionStoreFailure {
            kind: self.kind,
            scope: self.namespace.clone(),
            reason: reason.into(),
        }
    }

    fn read_state<T>(
        &self,
        op: impl FnOnce(&SessionStoreSnapshot) -> T,
    ) -> Result<T, RuntimeBrowserError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| self.error("session backend mutex poisoned"))?;
        let payload: Option<String> = connection
            .query_row(
                "SELECT payload FROM session_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row: &Row<'_>| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|reason: rusqlite::Error| self.error(reason.to_string()))?;

        let state = match payload {
            Some(payload) => serde_json::from_str(&payload).map_err(|reason: serde_json::Error| {
                self.error(reason.to_string())
            })?,
            None => SessionStoreSnapshot::default(),
        };

        Ok(op(&state))
    }

    fn with_state_mut<T>(
        &self,
        op: impl FnOnce(&mut SessionStoreSnapshot) -> Result<T, RuntimeBrowserError>,
    ) -> Result<T, RuntimeBrowserError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| self.error("session backend mutex poisoned"))?;
        let tx = connection
            .transaction()
            .map_err(|reason: rusqlite::Error| self.error(reason.to_string()))?;
        let payload: Option<String> = tx
            .query_row(
                "SELECT payload FROM session_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row: &Row<'_>| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|reason: rusqlite::Error| self.error(reason.to_string()))?;

        let mut state = match payload {
            Some(payload) => serde_json::from_str(&payload).map_err(|reason: serde_json::Error| {
                self.error(reason.to_string())
            })?,
            None => SessionStoreSnapshot::default(),
        };

        let outcome = op(&mut state);
        let should_persist = matches!(
            outcome.as_ref(),
            Ok(_) | Err(RuntimeBrowserError::ExpiredSession { .. })
        );
        if should_persist {
            let payload = serde_json::to_string(&state)
                .map_err(|reason: serde_json::Error| self.error(reason.to_string()))?;
            tx.execute(
                "INSERT INTO session_state (namespace, payload) VALUES (?1, ?2)
                 ON CONFLICT(namespace) DO UPDATE SET payload = excluded.payload",
                params![self.namespace.as_str(), payload],
            )
            .map_err(|reason: rusqlite::Error| self.error(reason.to_string()))?;
            tx.commit()
                .map_err(|reason: rusqlite::Error| self.error(reason.to_string()))?;
        }

        outcome
    }
}

fn live_database_path(
    kind: SessionStoreBackendKind,
    namespace: &str,
    root: PathBuf,
) -> PathBuf {
    root.join("browser")
        .join(session_backend_slug(kind))
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

fn session_backend_slug(kind: SessionStoreBackendKind) -> &'static str {
    match kind {
        SessionStoreBackendKind::Local => "local",
        SessionStoreBackendKind::Database => "database",
        SessionStoreBackendKind::Redis => "redis",
        SessionStoreBackendKind::Valkey => "valkey",
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
