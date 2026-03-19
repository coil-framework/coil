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
    store: davenda_core::SessionStoreTopology,
) -> SessionStoreBackendKind {
    match store {
        davenda_core::SessionStoreTopology::Memory => SessionStoreBackendKind::Local,
        davenda_core::SessionStoreTopology::Database => SessionStoreBackendKind::Database,
        davenda_core::SessionStoreTopology::Redis => SessionStoreBackendKind::Redis,
        davenda_core::SessionStoreTopology::Valkey => SessionStoreBackendKind::Valkey,
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
    fn issue(&self, record: BrowserSessionRecord);
    fn session(&self, session_id: &str) -> Option<BrowserSessionRecord>;
    fn delete(&self, session_id: &str);
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

    #[allow(dead_code)]
    pub(crate) fn test_only_shared_runtime(
        kind: SessionStoreBackendKind,
        scope: impl Into<String>,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        #[cfg(test)]
        {
            return super::shared::test_only_persistent_runtime(kind, scope.into());
        }

        #[cfg(not(test))]
        {
            let _ = kind;
            let _ = scope;
            panic!("test_only_shared_runtime is only available in test builds");
        }
    }

    #[cfg(not(test))]
    pub(crate) fn unconfigured_live_shared_runtime(
        kind: SessionStoreBackendKind,
        scope: impl Into<String>,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        // Live browser sessions must be configured explicitly; this is the
        // rejection backend for non-test builds.
        Arc::new(UnconfiguredLiveDistributedSessionStoreRuntime::new(
            kind,
            scope.into(),
        ))
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

    pub(super) fn issue(&self, record: BrowserSessionRecord) {
        self.runtime.issue(record);
    }

    pub(super) fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        self.runtime.session(session_id)
    }

    pub(super) fn delete(&self, session_id: &str) {
        self.runtime.delete(session_id);
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

#[cfg(not(test))]
#[derive(Debug)]
pub(super) struct UnconfiguredLiveDistributedSessionStoreRuntime {
    kind: SessionStoreBackendKind,
    scope: String,
}

#[cfg(not(test))]
impl UnconfiguredLiveDistributedSessionStoreRuntime {
    pub(super) fn new(kind: SessionStoreBackendKind, scope: String) -> Self {
        Self { kind, scope }
    }

    fn unsupported_message(&self) -> String {
        format!(
            "live browser session store `{kind:?}` for `{scope}` requires an explicit distributed runtime; file-backed shared state is test-only",
            kind = self.kind,
            scope = self.scope
        )
    }
}

#[cfg(not(test))]
impl DistributedSessionStoreRuntime for UnconfiguredLiveDistributedSessionStoreRuntime {
    fn issue(&self, _record: BrowserSessionRecord) {
        panic!("{}", self.unsupported_message());
    }

    fn session(&self, _session_id: &str) -> Option<BrowserSessionRecord> {
        panic!("{}", self.unsupported_message());
    }

    fn delete(&self, _session_id: &str) {
        panic!("{}", self.unsupported_message());
    }

    fn revoke(&self, _session_id: &str, _now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        panic!("{}", self.unsupported_message());
    }

    fn touch_active_session(
        &self,
        _session_id: &str,
        _idle_timeout: Duration,
        _now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        panic!("{}", self.unsupported_message());
    }

    fn is_shared_backend(&self) -> bool {
        false
    }

    fn supports_live_shared_state(&self) -> bool {
        false
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
        services: &davenda_core::SessionSecurityServices,
        backend_scope: &str,
    ) -> Result<(SessionStoreBackendKind, Self), BrowserHostBuildError> {
        match services.store {
            davenda_core::SessionStoreTopology::Memory => {
                Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost)
            }
            davenda_core::SessionStoreTopology::Database => Ok((
                SessionStoreBackendKind::Database,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Database,
                    DistributedSessionStoreClient::test_only_shared_runtime(
                        SessionStoreBackendKind::Database,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
            davenda_core::SessionStoreTopology::Redis => Ok((
                SessionStoreBackendKind::Redis,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Redis,
                    DistributedSessionStoreClient::test_only_shared_runtime(
                        SessionStoreBackendKind::Redis,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
            davenda_core::SessionStoreTopology::Valkey => Ok((
                SessionStoreBackendKind::Valkey,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Valkey,
                    DistributedSessionStoreClient::test_only_shared_runtime(
                        SessionStoreBackendKind::Valkey,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
        }
    }

    pub(super) fn with_client(
        services: &davenda_core::SessionSecurityServices,
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

    pub(super) fn issue(&mut self, record: BrowserSessionRecord) {
        match self {
            #[cfg(test)]
            Self::Local(state) => state.issue(record),
            Self::Distributed(client) => client.issue(record),
        }
    }

    pub(super) fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        match self {
            #[cfg(test)]
            Self::Local(state) => state.session(session_id),
            Self::Distributed(client) => client.session(session_id),
        }
    }

    pub(super) fn delete(&mut self, session_id: &str) {
        match self {
            #[cfg(test)]
            Self::Local(state) => {
                state.sessions.remove(session_id);
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
    host.sessions.issue(record);
    Ok(issued)
}
