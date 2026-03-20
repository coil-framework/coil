#![cfg(test)]
#![allow(dead_code)]
//! Test-only browser session persistence used by browser session tests.

use super::{
    BrowserInstant, BrowserSessionRecord, DistributedSessionStoreRuntime, RuntimeBrowserError,
    SessionStoreBackendKind,
};
#[cfg(test)]
use rusqlite::{Connection, OptionalExtension, Row, params};
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
#[cfg(test)]
use std::sync::OnceLock;
#[cfg(test)]
use std::time::Duration;

#[cfg(test)]
const SHARED_STATE_DIR_ENV: &str = "DAVENDA_SHARED_STATE_DIR";
#[cfg(test)]
const SHARED_STATE_NAMESPACE_ENV: &str = "DAVENDA_SHARED_BACKEND_NAMESPACE";

#[cfg(test)]
pub(crate) fn test_only_sqlite_shared_runtime(
    kind: SessionStoreBackendKind,
    namespace: impl Into<String>,
) -> Arc<dyn DistributedSessionStoreRuntime> {
    Arc::new(TestOnlySqliteSharedSessionStoreRuntime::new(
        kind,
        namespace.into(),
    ))
}

#[cfg(test)]
#[derive(Debug)]
struct TestOnlySqliteSharedSessionStoreRuntime {
    store: TestOnlySqliteSharedSessionStore,
}

#[cfg(test)]
impl TestOnlySqliteSharedSessionStoreRuntime {
    fn new(kind: SessionStoreBackendKind, namespace: String) -> Self {
        Self {
            store: TestOnlySqliteSharedSessionStore::open(kind, namespace),
        }
    }
}

#[cfg(test)]
impl DistributedSessionStoreRuntime for TestOnlySqliteSharedSessionStoreRuntime {
    fn issue(&self, record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError> {
        self.store
            .with_state_mut(|state| {
                state.issue(record);
                Ok(())
            })
            .expect("persistent session backend issue failed");
        Ok(())
    }

    fn session(
        &self,
        session_id: &str,
    ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
        Ok(self
            .store
            .read_state(|state| state.session(session_id))
            .expect("persistent session backend lookup failed"))
    }

    fn delete(&self, session_id: &str) -> Result<(), RuntimeBrowserError> {
        self.store
            .with_state_mut(|state| {
                state.sessions.remove(session_id);
                Ok(())
            })
            .expect("persistent session backend delete failed");
        Ok(())
    }

    fn revoke(&self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        self.store
            .with_state_mut(|state| state.revoke(session_id, now))
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
        false
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct SessionStoreSnapshot {
    sessions: BTreeMap<String, BrowserSessionRecord>,
}

#[cfg(test)]
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

#[cfg(test)]
#[derive(Debug)]
struct TestOnlySqliteSharedSessionStore {
    connection: Arc<Mutex<Connection>>,
    namespace: String,
}

#[cfg(test)]
impl TestOnlySqliteSharedSessionStore {
    fn open(kind: SessionStoreBackendKind, namespace: String) -> Self {
        let namespace = std::env::var(SHARED_STATE_NAMESPACE_ENV).unwrap_or(namespace);
        let path = test_only_sqlite_database_path(kind, &namespace);
        let connection = shared_connection(&path);

        Self {
            connection,
            namespace,
        }
    }

    fn read_state<T>(
        &self,
        op: impl FnOnce(&SessionStoreSnapshot) -> T,
    ) -> Result<T, RuntimeBrowserError> {
        let connection = self
            .connection
            .lock()
            .expect("session backend mutex poisoned");
        let payload = connection
            .query_row(
                "SELECT payload FROM session_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row: &Row<'_>| row.get::<_, String>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read persistent session backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let state = match payload {
            Some(payload) => serde_json::from_str(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize persistent session backend state for `{}`: {error}",
                    self.namespace
                )
            }),
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
            .expect("session backend mutex poisoned");
        let tx = connection.transaction().unwrap_or_else(|error| {
            panic!(
                "failed to start persistent session backend transaction for `{}`: {error}",
                self.namespace
            )
        });
        let payload = tx
            .query_row(
                "SELECT payload FROM session_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row: &Row<'_>| row.get::<_, String>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read persistent session backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let mut state = match payload {
            Some(payload) => serde_json::from_str(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize persistent session backend state for `{}`: {error}",
                    self.namespace
                )
            }),
            None => SessionStoreSnapshot::default(),
        };

        let outcome = op(&mut state);
        let should_persist = matches!(
            outcome.as_ref(),
            Ok(_) | Err(RuntimeBrowserError::ExpiredSession { .. })
        );
        if should_persist {
            let payload = serde_json::to_string(&state).unwrap_or_else(|error| {
                panic!(
                    "failed to serialize persistent session backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.execute(
                "INSERT INTO session_state (namespace, payload) VALUES (?1, ?2)
                 ON CONFLICT(namespace) DO UPDATE SET payload = excluded.payload",
                params![self.namespace.as_str(), payload],
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to persist session backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.commit().unwrap_or_else(|error| {
                panic!(
                    "failed to commit persistent session backend state for `{}`: {error}",
                    self.namespace
                )
            });
        }

        outcome
    }
}

#[cfg(test)]
fn shared_connection(path: &PathBuf) -> Arc<Mutex<Connection>> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Mutex<Connection>>>>> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("persistent session backend registry mutex poisoned");

    guard
        .entry(path.clone())
        .or_insert_with(|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                    panic!(
                        "failed to create persistent session backend directory `{}`: {error}",
                        parent.display()
                    )
                });
            }

            let connection = Connection::open(path).unwrap_or_else(|error| {
                panic!(
                    "failed to open persistent session backend `{}`: {error}",
                    path.display()
                )
            });
            connection
                .execute_batch(
                    r#"
                    PRAGMA journal_mode = WAL;
                    PRAGMA synchronous = NORMAL;
                    CREATE TABLE IF NOT EXISTS session_state (
                        namespace TEXT PRIMARY KEY,
                        payload TEXT NOT NULL
                    );
                    "#,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "failed to initialize persistent session backend `{}`: {error}",
                        path.display()
                    )
                });
            connection
                .busy_timeout(Duration::from_secs(5))
                .unwrap_or_else(|error| {
                    panic!(
                        "failed to configure persistent session backend busy timeout `{}`: {error}",
                        path.display()
                    )
                });
            Arc::new(Mutex::new(connection))
        })
        .clone()
}

#[cfg(test)]
fn test_only_sqlite_database_path(kind: SessionStoreBackendKind, namespace: &str) -> PathBuf {
    test_only_sqlite_shared_state_root()
        .join("browser")
        .join(session_backend_slug(kind))
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

#[cfg(test)]
fn test_only_sqlite_shared_state_root() -> PathBuf {
    std::env::var_os(SHARED_STATE_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!("davenda-shared-{}", std::process::id()))
        })
}

#[cfg(test)]
fn session_backend_slug(kind: SessionStoreBackendKind) -> &'static str {
    match kind {
        SessionStoreBackendKind::Local => "local",
        SessionStoreBackendKind::Database => "database",
        SessionStoreBackendKind::Redis => "redis",
        SessionStoreBackendKind::Valkey => "valkey",
    }
}

#[cfg(test)]
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
