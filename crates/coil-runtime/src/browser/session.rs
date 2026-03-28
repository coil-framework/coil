use super::support::{issue_session_id, validate_browser_value};
use super::*;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionStoreBackendKind {
    Local,
    Database,
    Redis,
    Valkey,
}

fn session_store_backend_kind(
    store: coil_core::SessionStoreTopology,
) -> SessionStoreBackendKind {
    match store {
        coil_core::SessionStoreTopology::Memory => SessionStoreBackendKind::Local,
        coil_core::SessionStoreTopology::Database => SessionStoreBackendKind::Database,
        coil_core::SessionStoreTopology::Redis => SessionStoreBackendKind::Redis,
        coil_core::SessionStoreTopology::Valkey => SessionStoreBackendKind::Valkey,
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct BrowserInstant(u64);

impl BrowserInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn as_unix_seconds(self) -> u64 {
        self.0
    }

    pub fn saturating_add(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration.as_secs()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionIssueRequest {
    pub principal_id: Option<String>,
}

impl SessionIssueRequest {
    pub const fn new() -> Self {
        Self { principal_id: None }
    }

    pub fn for_principal(
        mut self,
        principal_id: impl Into<String>,
    ) -> Result<Self, RuntimeBrowserError> {
        self.principal_id = Some(validate_browser_value("principal_id", principal_id.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserSessionStatus {
    Active,
    IdleExpired,
    AbsoluteExpired,
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BrowserSessionRecord {
    pub session_id: String,
    pub principal_id: Option<String>,
    pub issued_at: BrowserInstant,
    pub last_seen_at: BrowserInstant,
    pub idle_expires_at: BrowserInstant,
    pub absolute_expires_at: BrowserInstant,
    pub revoked_at: Option<BrowserInstant>,
}

impl BrowserSessionRecord {
    pub fn status_at(&self, now: BrowserInstant) -> BrowserSessionStatus {
        if self.revoked_at.is_some() {
            BrowserSessionStatus::Revoked
        } else if now.as_unix_seconds() > self.absolute_expires_at.as_unix_seconds() {
            BrowserSessionStatus::AbsoluteExpired
        } else if now.as_unix_seconds() > self.idle_expires_at.as_unix_seconds() {
            BrowserSessionStatus::IdleExpired
        } else {
            BrowserSessionStatus::Active
        }
    }
}

pub trait DistributedSessionStoreRuntime: Send + Sync + 'static {
    fn issue(&self, record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError>;
    fn session(
        &self,
        session_id: &str,
    ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError>;
    fn delete(&self, session_id: &str) -> Result<(), RuntimeBrowserError>;
    fn revoke(&self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError>;
    fn touch_active_session(
        &self,
        session_id: &str,
        idle_timeout: Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError>;
    fn is_shared_backend(&self) -> bool;
    fn supports_live_shared_state(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct DistributedSessionStoreClient {
    kind: SessionStoreBackendKind,
    runtime: Arc<dyn DistributedSessionStoreRuntime>,
}

impl DistributedSessionStoreClient {
    pub fn new(
        kind: SessionStoreBackendKind,
        runtime: Arc<dyn DistributedSessionStoreRuntime>,
    ) -> Self {
        Self { kind, runtime }
    }

    #[cfg(test)]
    pub(crate) fn test_only_sqlite_shared_runtime(
        kind: SessionStoreBackendKind,
        scope: impl Into<String>,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        super::testing::test_only_sqlite_shared_runtime(kind, scope.into())
    }

    pub fn kind(&self) -> SessionStoreBackendKind {
        self.kind
    }

    pub fn is_shared(&self) -> bool {
        self.runtime.is_shared_backend()
    }

    pub fn supports_live_shared_state(&self) -> bool {
        self.runtime.supports_live_shared_state()
    }

    pub(super) fn issue(&self, record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError> {
        self.runtime.issue(record)
    }

    pub(super) fn session(
        &self,
        session_id: &str,
    ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
        self.runtime.session(session_id)
    }

    pub(super) fn delete(&self, session_id: &str) -> Result<(), RuntimeBrowserError> {
        self.runtime.delete(session_id)
    }

    pub(super) fn revoke(
        &self,
        session_id: &str,
        now: BrowserInstant,
    ) -> Result<(), RuntimeBrowserError> {
        self.runtime.revoke(session_id, now)
    }

    pub(super) fn touch_active_session(
        &self,
        session_id: &str,
        idle_timeout: Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        self.runtime
            .touch_active_session(session_id, idle_timeout, now)
    }
}

impl std::fmt::Debug for DistributedSessionStoreClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DistributedSessionStoreClient")
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub(super) enum SessionStoreBackend {
    #[cfg(test)]
    Local(testing::SessionStoreState),
    Distributed(DistributedSessionStoreClient),
}

impl SessionStoreBackend {
    #[cfg(test)]
    pub(super) fn shared(
        customer_app: &str,
        services: &coil_core::SessionSecurityServices,
        backend_scope: &str,
    ) -> Result<(SessionStoreBackendKind, Self), BrowserHostBuildError> {
        match services.store {
            coil_core::SessionStoreTopology::Memory => {
                Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost)
            }
            coil_core::SessionStoreTopology::Database => Ok((
                SessionStoreBackendKind::Database,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Database,
                    DistributedSessionStoreClient::test_only_sqlite_shared_runtime(
                        SessionStoreBackendKind::Database,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
            coil_core::SessionStoreTopology::Redis => Ok((
                SessionStoreBackendKind::Redis,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Redis,
                    DistributedSessionStoreClient::test_only_sqlite_shared_runtime(
                        SessionStoreBackendKind::Redis,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
            coil_core::SessionStoreTopology::Valkey => Ok((
                SessionStoreBackendKind::Valkey,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Valkey,
                    DistributedSessionStoreClient::test_only_sqlite_shared_runtime(
                        SessionStoreBackendKind::Valkey,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
        }
    }

    pub(super) fn with_client(
        services: &coil_core::SessionSecurityServices,
        client: DistributedSessionStoreClient,
    ) -> Result<(SessionStoreBackendKind, Self), BrowserHostBuildError> {
        let expected = session_store_backend_kind(services.store);
        if expected == SessionStoreBackendKind::Local {
            return Err(BrowserHostBuildError::MemoryStoreCannotUseDistributedClient);
        }

        if client.kind() != expected {
            return Err(BrowserHostBuildError::SessionStoreClientKindMismatch {
                expected,
                actual: client.kind(),
            });
        }

        Ok((expected, Self::Distributed(client)))
    }

    pub(super) fn is_shared(&self) -> bool {
        match self {
            #[cfg(test)]
            Self::Local(_) => false,
            Self::Distributed(client) => client.is_shared(),
        }
    }

    pub(super) fn is_live_shared_state_supported(&self) -> bool {
        match self {
            #[cfg(test)]
            Self::Local(_) => false,
            Self::Distributed(client) => client.supports_live_shared_state(),
        }
    }

    pub(super) fn issue(
        &mut self,
        record: BrowserSessionRecord,
    ) -> Result<(), RuntimeBrowserError> {
        match self {
            #[cfg(test)]
            Self::Local(state) => {
                state.issue(record);
                Ok(())
            }
            Self::Distributed(client) => client.issue(record),
        }
    }

    pub(super) fn session(
        &self,
        session_id: &str,
    ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
        match self {
            #[cfg(test)]
            Self::Local(state) => Ok(state.session(session_id)),
            Self::Distributed(client) => client.session(session_id),
        }
    }

    pub(super) fn delete(&mut self, session_id: &str) -> Result<(), RuntimeBrowserError> {
        match self {
            #[cfg(test)]
            Self::Local(state) => {
                state.sessions.remove(session_id);
                Ok(())
            }
            Self::Distributed(client) => client.delete(session_id),
        }
    }

    pub(super) fn revoke(
        &mut self,
        session_id: &str,
        now: BrowserInstant,
    ) -> Result<(), RuntimeBrowserError> {
        match self {
            #[cfg(test)]
            Self::Local(state) => state.revoke(session_id, now),
            Self::Distributed(client) => client.revoke(session_id, now),
        }
    }

    pub(super) fn touch_active_session(
        &mut self,
        session_id: &str,
        idle_timeout: Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        match self {
            #[cfg(test)]
            Self::Local(state) => state.touch_active_session(session_id, idle_timeout, now),
            Self::Distributed(client) => client.touch_active_session(session_id, idle_timeout, now),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedBrowserSession {
    pub record: BrowserSessionRecord,
    pub cookie_value: String,
    pub set_cookie_header: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotatedBrowserSession {
    pub previous_session_id: String,
    pub issued: IssuedBrowserSession,
}

pub(super) fn issue_session(
    host: &mut BrowserHost,
    request: SessionIssueRequest,
    cookie_secret: &[u8],
    now: BrowserInstant,
) -> Result<IssuedBrowserSession, RuntimeBrowserError> {
    let session_id = issue_session_id();
    let record = BrowserSessionRecord {
        session_id: session_id.clone(),
        principal_id: request.principal_id,
        issued_at: now,
        last_seen_at: now,
        idle_expires_at: now.saturating_add(host.services.sessions.idle_timeout),
        absolute_expires_at: now.saturating_add(host.services.sessions.absolute_timeout),
        revoked_at: None,
    };
    let issued = host.issue_cookie_for_record(record.clone(), cookie_secret)?;
    host.sessions.issue(record)?;
    Ok(issued)
}
