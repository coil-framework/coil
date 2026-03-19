use super::session::SessionStoreBackend;
use super::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct SessionStoreState {
    pub(crate) sessions: BTreeMap<String, BrowserSessionRecord>,
}

#[cfg(test)]
impl SessionStoreState {
    pub(crate) fn issue(&mut self, record: BrowserSessionRecord) {
        self.sessions.insert(record.session_id.clone(), record);
    }

    pub(crate) fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        self.sessions.get(session_id).cloned()
    }

    pub(crate) fn revoke(
        &mut self,
        session_id: &str,
        now: BrowserInstant,
    ) -> Result<(), RuntimeBrowserError> {
        let existing = self.sessions.get_mut(session_id).ok_or_else(|| {
            RuntimeBrowserError::UnknownSession {
                session_id: session_id.to_string(),
            }
        })?;
        existing.revoked_at = Some(now);
        Ok(())
    }

    pub(crate) fn touch_active_session(
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
#[derive(Debug, Default)]
struct SharedDistributedSessionStoreRuntime {
    state: Mutex<SessionStoreState>,
}

#[cfg(test)]
impl SharedDistributedSessionStoreRuntime {
    fn new() -> Self {
        Self {
            state: Mutex::new(SessionStoreState::default()),
        }
    }
}

#[cfg(test)]
impl DistributedSessionStoreRuntime for SharedDistributedSessionStoreRuntime {
    fn issue(&self, record: BrowserSessionRecord) {
        let mut guard = self.state.lock().expect("session backend mutex poisoned");
        guard.issue(record);
    }

    fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        let guard = self.state.lock().expect("session backend mutex poisoned");
        guard.session(session_id)
    }

    fn delete(&self, session_id: &str) {
        let mut guard = self.state.lock().expect("session backend mutex poisoned");
        guard.sessions.remove(session_id);
    }

    fn revoke(&self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        let mut guard = self.state.lock().expect("session backend mutex poisoned");
        guard.revoke(session_id, now)
    }

    fn touch_active_session(
        &self,
        session_id: &str,
        idle_timeout: std::time::Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        let mut guard = self.state.lock().expect("session backend mutex poisoned");
        guard.touch_active_session(session_id, idle_timeout, now)
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[cfg(test)]
pub(crate) fn shared_test_runtime(
    kind: SessionStoreBackendKind,
    scope: String,
) -> Arc<dyn DistributedSessionStoreRuntime> {
    static REGISTRY: OnceLock<Mutex<BTreeMap<String, Arc<dyn DistributedSessionStoreRuntime>>>> =
        OnceLock::new();

    let key = format!("{}:{kind:?}:{scope}", test_scope());
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry
        .lock()
        .expect("test session store registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| Arc::new(SharedDistributedSessionStoreRuntime::new()))
        .clone()
}

#[cfg(test)]
fn test_scope() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed-test")
        .to_string()
}

#[cfg(test)]
impl DistributedSessionStoreClient {
    pub(crate) fn local_for_testing(kind: SessionStoreBackendKind) -> Self {
        Self::new(kind, Arc::new(SharedDistributedSessionStoreRuntime::new()))
    }

    pub(crate) fn shared_runtime(
        kind: SessionStoreBackendKind,
        scope: impl Into<String>,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        shared_test_runtime(kind, scope.into())
    }
}

#[cfg(test)]
impl SessionStoreBackend {
    pub(crate) fn local(
        _customer_app: &str,
        services: &davenda_core::SessionSecurityServices,
    ) -> (SessionStoreBackendKind, Self) {
        match services.store {
            davenda_core::SessionStoreTopology::Memory => (
                SessionStoreBackendKind::Local,
                Self::Local(SessionStoreState::default()),
            ),
            davenda_core::SessionStoreTopology::Database => (
                SessionStoreBackendKind::Database,
                Self::Distributed(DistributedSessionStoreClient::local_for_testing(
                    SessionStoreBackendKind::Database,
                )),
            ),
            davenda_core::SessionStoreTopology::Redis => (
                SessionStoreBackendKind::Redis,
                Self::Distributed(DistributedSessionStoreClient::local_for_testing(
                    SessionStoreBackendKind::Redis,
                )),
            ),
            davenda_core::SessionStoreTopology::Valkey => (
                SessionStoreBackendKind::Valkey,
                Self::Distributed(DistributedSessionStoreClient::local_for_testing(
                    SessionStoreBackendKind::Valkey,
                )),
            ),
        }
    }
}

#[cfg(test)]
impl BrowserHost {
    pub(crate) fn local_for_testing(
        customer_app: String,
        services: BrowserSecurityServices,
    ) -> Self {
        let (session_store_kind, sessions) =
            SessionStoreBackend::local(&customer_app, &services.sessions);
        Self {
            customer_app,
            services,
            session_store_kind,
            sessions,
        }
    }
}
