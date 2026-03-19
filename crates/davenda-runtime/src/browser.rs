use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use davenda_core::BrowserSecurityError;

mod shared;

#[cfg(test)]
use std::sync::OnceLock;

const FLASH_COOKIE_MAX_AGE_SECS: u64 = 300;
static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

#[derive(Debug, Clone, Default)]
struct SessionStoreState {
    sessions: BTreeMap<String, BrowserSessionRecord>,
}

impl SessionStoreState {
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
        idle_timeout: Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        let record = self.sessions.get_mut(session_id).ok_or_else(|| {
            RuntimeBrowserError::UnknownSession {
                session_id: session_id.to_string(),
            }
        })?;

        match record.status_at(now) {
            BrowserSessionStatus::Active => {
                record.last_seen_at = now;
                record.idle_expires_at = now.saturating_add(idle_timeout);
                Ok(record.principal_id.clone())
            }
            BrowserSessionStatus::IdleExpired | BrowserSessionStatus::AbsoluteExpired => {
                self.sessions.remove(session_id);
                Err(RuntimeBrowserError::ExpiredSession {
                    session_id: session_id.to_string(),
                })
            }
            BrowserSessionStatus::Revoked => Err(RuntimeBrowserError::RevokedSession {
                session_id: session_id.to_string(),
            }),
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
}

#[derive(Debug)]
struct SharedDistributedSessionStoreRuntime {
    state: Mutex<SessionStoreState>,
}

impl SharedDistributedSessionStoreRuntime {
    fn new() -> Self {
        Self {
            state: Mutex::new(SessionStoreState::default()),
        }
    }
}

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
        idle_timeout: Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        let mut guard = self.state.lock().expect("session backend mutex poisoned");
        guard.touch_active_session(session_id, idle_timeout, now)
    }

    fn is_shared_backend(&self) -> bool {
        true
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
    #[allow(dead_code)]
    pub(crate) fn in_memory(kind: SessionStoreBackendKind) -> Self {
        Self::local_for_testing(kind)
    }

    #[cfg(test)]
    #[doc(hidden)]
    pub(crate) fn local_for_testing(kind: SessionStoreBackendKind) -> Self {
        Self::new(kind, Arc::new(SharedDistributedSessionStoreRuntime::new()))
    }

    #[cfg(test)]
    pub(crate) fn shared_runtime(
        kind: SessionStoreBackendKind,
        scope: impl Into<String>,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        shared_test_runtime(kind, scope.into())
    }

    #[cfg(not(test))]
    pub(crate) fn shared_runtime(
        kind: SessionStoreBackendKind,
        scope: impl Into<String>,
    ) -> Arc<dyn DistributedSessionStoreRuntime> {
        shared::persistent_runtime(kind, scope.into())
    }

    pub fn kind(&self) -> SessionStoreBackendKind {
        self.kind
    }

    pub fn is_shared(&self) -> bool {
        self.runtime.is_shared_backend()
    }

    fn issue(&self, record: BrowserSessionRecord) {
        self.runtime.issue(record);
    }

    fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        self.runtime.session(session_id)
    }

    fn delete(&self, session_id: &str) {
        self.runtime.delete(session_id);
    }

    fn revoke(&self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        self.runtime.revoke(session_id, now)
    }

    fn touch_active_session(
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

#[cfg(test)]
fn shared_test_runtime(
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

#[derive(Debug, Clone)]
enum SessionStoreBackend {
    Local(SessionStoreState),
    Distributed(DistributedSessionStoreClient),
}

impl SessionStoreBackend {
    #[cfg(test)]
    fn shared(
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
                    DistributedSessionStoreClient::shared_runtime(
                        SessionStoreBackendKind::Database,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
            davenda_core::SessionStoreTopology::Redis => Ok((
                SessionStoreBackendKind::Redis,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Redis,
                    DistributedSessionStoreClient::shared_runtime(
                        SessionStoreBackendKind::Redis,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
            davenda_core::SessionStoreTopology::Valkey => Ok((
                SessionStoreBackendKind::Valkey,
                Self::Distributed(DistributedSessionStoreClient::new(
                    SessionStoreBackendKind::Valkey,
                    DistributedSessionStoreClient::shared_runtime(
                        SessionStoreBackendKind::Valkey,
                        format!("{backend_scope}:{customer_app}"),
                    ),
                )),
            )),
        }
    }

    #[cfg(test)]
    fn local(
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

    fn with_client(
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

    fn is_shared(&self) -> bool {
        match self {
            Self::Local(_) => false,
            Self::Distributed(client) => client.is_shared(),
        }
    }

    fn issue(&mut self, record: BrowserSessionRecord) {
        match self {
            Self::Local(state) => state.issue(record),
            Self::Distributed(client) => client.issue(record),
        }
    }

    fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        match self {
            Self::Local(state) => state.session(session_id),
            Self::Distributed(client) => client.session(session_id),
        }
    }

    fn delete(&mut self, session_id: &str) {
        match self {
            Self::Local(state) => {
                state.sessions.remove(session_id);
            }
            Self::Distributed(client) => client.delete(session_id),
        }
    }

    fn revoke(&mut self, session_id: &str, now: BrowserInstant) -> Result<(), RuntimeBrowserError> {
        match self {
            Self::Local(state) => state.revoke(session_id, now),
            Self::Distributed(client) => client.revoke(session_id, now),
        }
    }

    fn touch_active_session(
        &mut self,
        session_id: &str,
        idle_timeout: Duration,
        now: BrowserInstant,
    ) -> Result<Option<String>, RuntimeBrowserError> {
        match self {
            Self::Local(state) => state.touch_active_session(session_id, idle_timeout, now),
            Self::Distributed(client) => client.touch_active_session(session_id, idle_timeout, now),
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlashLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl FlashLevel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeBrowserError> {
        match value {
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            other => Err(RuntimeBrowserError::InvalidFlashLevel {
                level: other.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashMessage {
    pub level: FlashLevel,
    pub text: String,
}

impl FlashMessage {
    pub fn new(level: FlashLevel, text: impl Into<String>) -> Result<Self, RuntimeBrowserError> {
        let text = validate_browser_value("flash_message", text.into())?;
        Ok(Self { level, text })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBrowserRequest {
    pub session: SessionContext,
    pub principal_id: Option<String>,
    pub flash_messages: Vec<FlashMessage>,
    pub response_cookies: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeBrowserError {
    #[error("browser value `{field}` must not be empty")]
    EmptyValue { field: &'static str },
    #[error("session cookie failed validation: {reason}")]
    InvalidSessionCookie { reason: String },
    #[error("flash cookie failed validation: {reason}")]
    InvalidFlashCookie { reason: String },
    #[error("session `{session_id}` is not present in the server-side store")]
    UnknownSession { session_id: String },
    #[error("session `{session_id}` has expired")]
    ExpiredSession { session_id: String },
    #[error("session `{session_id}` has been revoked")]
    RevokedSession { session_id: String },
    #[error("flash cookie payload is malformed")]
    InvalidFlashPayload,
    #[error("flash cookie contains unknown level `{level}`")]
    InvalidFlashLevel { level: String },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BrowserHostBuildError {
    #[error("memory session stores are test-only and cannot back a live browser host")]
    MemoryStoreRequiresTestOnlyBrowserHost,
    #[error("memory session stores cannot use a distributed session client")]
    MemoryStoreCannotUseDistributedClient,
    #[error("session store client kind mismatch: expected `{expected:?}`, got `{actual:?}`")]
    SessionStoreClientKindMismatch {
        expected: SessionStoreBackendKind,
        actual: SessionStoreBackendKind,
    },
}

#[derive(Debug, Clone)]
pub struct BrowserHost {
    pub customer_app: String,
    pub services: BrowserSecurityServices,
    session_store_kind: SessionStoreBackendKind,
    sessions: SessionStoreBackend,
}

impl BrowserHost {
    #[cfg(test)]
    pub(crate) fn new_with_scope(
        customer_app: String,
        services: BrowserSecurityServices,
        backend_scope: impl Into<String>,
    ) -> Result<Self, BrowserHostBuildError> {
        let backend_scope = backend_scope.into();
        let (session_store_kind, sessions) =
            SessionStoreBackend::shared(&customer_app, &services.sessions, &backend_scope)?;
        Ok(Self {
            customer_app,
            services,
            session_store_kind,
            sessions,
        })
    }

    #[cfg(test)]
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

    pub fn with_session_store_client(
        customer_app: String,
        services: BrowserSecurityServices,
        client: DistributedSessionStoreClient,
    ) -> Result<Self, BrowserHostBuildError> {
        let (session_store_kind, sessions) =
            SessionStoreBackend::with_client(&services.sessions, client)?;
        Ok(Self {
            customer_app,
            services,
            session_store_kind,
            sessions,
        })
    }

    pub fn session_store_kind(&self) -> SessionStoreBackendKind {
        self.session_store_kind
    }

    pub fn session_store_is_shared(&self) -> bool {
        self.sessions.is_shared()
    }

    pub fn issue_session(
        &mut self,
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
            idle_expires_at: now.saturating_add(self.services.sessions.idle_timeout),
            absolute_expires_at: now.saturating_add(self.services.sessions.absolute_timeout),
            revoked_at: None,
        };
        let issued = self.issue_cookie_for_record(record.clone(), cookie_secret)?;
        self.sessions.issue(record);
        Ok(issued)
    }

    pub fn rotate_session(
        &mut self,
        session_id: &str,
        cookie_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<RotatedBrowserSession, RuntimeBrowserError> {
        let session_id = validate_browser_value("session_id", session_id.to_string())?;
        let existing = self.sessions.session(&session_id).ok_or_else(|| {
            RuntimeBrowserError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;
        let principal_id = match existing.status_at(now) {
            BrowserSessionStatus::Active => {
                self.sessions.revoke(&session_id, now)?;
                existing.principal_id.clone()
            }
            BrowserSessionStatus::IdleExpired | BrowserSessionStatus::AbsoluteExpired => {
                self.sessions.delete(&session_id);
                return Err(RuntimeBrowserError::ExpiredSession { session_id });
            }
            BrowserSessionStatus::Revoked => {
                return Err(RuntimeBrowserError::RevokedSession { session_id });
            }
        };

        let issued =
            self.issue_session(SessionIssueRequest { principal_id }, cookie_secret, now)?;
        Ok(RotatedBrowserSession {
            previous_session_id: session_id,
            issued,
        })
    }

    pub fn revoke_session(
        &mut self,
        session_id: &str,
        now: BrowserInstant,
    ) -> Result<(), RuntimeBrowserError> {
        let session_id = validate_browser_value("session_id", session_id.to_string())?;
        self.sessions.revoke(&session_id, now)
    }

    pub fn issue_csrf_token(
        &self,
        csrf_secret: &[u8],
        session_id: &str,
        action: &str,
    ) -> Result<String, RuntimeBrowserError> {
        let session_id = validate_browser_value("session_id", session_id.to_string())?;
        let action = validate_browser_value("action", action.to_string())?;
        self.services
            .csrf
            .issue_token(csrf_secret, &session_id, &action)
            .map_err(map_session_cookie_error)
    }

    pub fn issue_flash_cookie(
        &self,
        cookie_secret: &[u8],
        messages: &[FlashMessage],
    ) -> Result<String, RuntimeBrowserError> {
        if messages.is_empty() {
            return Err(RuntimeBrowserError::EmptyValue {
                field: "flash_messages",
            });
        }

        let payload = serialize_flash_messages(messages)?;
        let value = self
            .services
            .sessions
            .flash_cookie
            .protect(cookie_secret, &payload)
            .map_err(map_flash_cookie_error)?;
        Ok(self
            .services
            .sessions
            .flash_cookie
            .render_set_cookie(&value, Some(Duration::from_secs(FLASH_COOKIE_MAX_AGE_SECS))))
    }

    pub fn clear_flash_cookie_header(&self) -> String {
        self.services
            .sessions
            .flash_cookie
            .render_set_cookie("", Some(Duration::from_secs(0)))
    }

    pub fn session(&self, session_id: &str) -> Option<BrowserSessionRecord> {
        self.sessions.session(session_id)
    }

    pub fn resolve_request(
        &mut self,
        request: &RequestInput,
        cookie_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<ResolvedBrowserRequest, RuntimeBrowserError> {
        let mut response_cookies = Vec::new();
        let flash_messages = match request.flash_cookie.as_deref() {
            Some(cookie) => {
                let messages = self.consume_flash_cookie(cookie_secret, cookie)?;
                response_cookies.push(self.clear_flash_cookie_header());
                messages
            }
            None => Vec::new(),
        };

        let mut resolved_from_cookie = false;
        let session_id = if let Some(session_id) = request.session_id.as_ref() {
            Some(validate_browser_value("session_id", session_id.clone())?)
        } else if let Some(cookie) = request.session_cookie.as_deref() {
            resolved_from_cookie = true;
            Some(self.verify_session_cookie(cookie_secret, cookie)?)
        } else {
            None
        };

        let Some(session_id) = session_id else {
            return Ok(ResolvedBrowserRequest {
                session: SessionContext {
                    session_id: None,
                    resolved_from_cookie,
                },
                principal_id: None,
                flash_messages,
                response_cookies,
            });
        };

        let (principal_id, refreshed_cookie) =
            self.touch_active_session(&session_id, cookie_secret, now)?;
        response_cookies.push(refreshed_cookie);

        Ok(ResolvedBrowserRequest {
            session: SessionContext {
                session_id: Some(session_id),
                resolved_from_cookie,
            },
            principal_id,
            flash_messages,
            response_cookies,
        })
    }

    fn issue_cookie_for_record(
        &self,
        record: BrowserSessionRecord,
        cookie_secret: &[u8],
    ) -> Result<IssuedBrowserSession, RuntimeBrowserError> {
        let cookie_value = self
            .services
            .sessions
            .session_cookie
            .protect(cookie_secret, &record.session_id)
            .map_err(map_session_cookie_error)?;
        let set_cookie_header = self
            .services
            .sessions
            .session_cookie
            .render_set_cookie(&cookie_value, Some(self.services.sessions.idle_timeout));
        Ok(IssuedBrowserSession {
            record,
            cookie_value,
            set_cookie_header,
        })
    }

    fn verify_session_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<String, RuntimeBrowserError> {
        self.services
            .sessions
            .session_cookie
            .unprotect(cookie_secret, cookie)
            .map_err(map_session_cookie_error)
    }

    fn consume_flash_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<Vec<FlashMessage>, RuntimeBrowserError> {
        let payload = self
            .services
            .sessions
            .flash_cookie
            .unprotect(cookie_secret, cookie)
            .map_err(map_flash_cookie_error)?;
        deserialize_flash_messages(&payload)
    }

    fn touch_active_session(
        &mut self,
        session_id: &str,
        cookie_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<(Option<String>, String), RuntimeBrowserError> {
        let principal_id = self.sessions.touch_active_session(
            session_id,
            self.services.sessions.idle_timeout,
            now,
        )?;
        let cookie_value = self
            .services
            .sessions
            .session_cookie
            .protect(cookie_secret, session_id)
            .map_err(map_session_cookie_error)?;
        let cookie_header = self
            .services
            .sessions
            .session_cookie
            .render_set_cookie(&cookie_value, Some(self.services.sessions.idle_timeout));
        Ok((principal_id, cookie_header))
    }
}

fn validate_browser_value(
    field: &'static str,
    value: String,
) -> Result<String, RuntimeBrowserError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RuntimeBrowserError::EmptyValue { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn issue_session_id() -> String {
    let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    let mut output = String::with_capacity(40);
    let _ = write!(output, "{nanos:032x}{sequence:08x}");
    output
}

fn serialize_flash_messages(messages: &[FlashMessage]) -> Result<String, RuntimeBrowserError> {
    let mut payload = String::new();
    for message in messages {
        if message.text.trim().is_empty() {
            return Err(RuntimeBrowserError::EmptyValue {
                field: "flash_message",
            });
        }

        payload.push_str(message.level.as_str());
        payload.push(':');
        payload.push_str(&message.text.len().to_string());
        payload.push(':');
        payload.push_str(&message.text);
        payload.push('|');
    }

    Ok(payload)
}

fn deserialize_flash_messages(payload: &str) -> Result<Vec<FlashMessage>, RuntimeBrowserError> {
    let mut remaining = payload;
    let mut messages = Vec::new();

    while !remaining.is_empty() {
        let Some(level_sep) = remaining.find(':') else {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        };
        let level = &remaining[..level_sep];
        remaining = &remaining[level_sep + 1..];

        let Some(length_sep) = remaining.find(':') else {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        };
        let length: usize = remaining[..length_sep]
            .parse()
            .map_err(|_| RuntimeBrowserError::InvalidFlashPayload)?;
        remaining = &remaining[length_sep + 1..];

        if remaining.len() < length + 1 {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        }

        let message = &remaining[..length];
        let separator = &remaining[length..length + 1];
        if separator != "|" {
            return Err(RuntimeBrowserError::InvalidFlashPayload);
        }

        messages.push(FlashMessage::new(FlashLevel::parse(level)?, message)?);
        remaining = &remaining[length + 1..];
    }

    Ok(messages)
}

fn map_session_cookie_error(error: BrowserSecurityError) -> RuntimeBrowserError {
    RuntimeBrowserError::InvalidSessionCookie {
        reason: error.to_string(),
    }
}

fn map_flash_cookie_error(error: BrowserSecurityError) -> RuntimeBrowserError {
    RuntimeBrowserError::InvalidFlashCookie {
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_core::{
        BrowserSecurityServices, CookiePolicy, CookieProtection, CsrfProtection,
        SessionSecurityServices, SessionStoreTopology,
    };

    fn services(store: SessionStoreTopology) -> BrowserSecurityServices {
        BrowserSecurityServices {
            sessions: SessionSecurityServices {
                store,
                idle_timeout: Duration::from_secs(300),
                absolute_timeout: Duration::from_secs(3600),
                session_cookie: CookiePolicy {
                    name: "session".to_string(),
                    domain: None,
                    path: "/".to_string(),
                    same_site: davenda_config::SameSitePolicy::Lax,
                    secure: true,
                    http_only: true,
                    protection: CookieProtection::Signed,
                },
                flash_cookie: CookiePolicy {
                    name: "flash".to_string(),
                    domain: None,
                    path: "/".to_string(),
                    same_site: davenda_config::SameSitePolicy::Lax,
                    secure: true,
                    http_only: true,
                    protection: CookieProtection::Signed,
                },
            },
            csrf: CsrfProtection {
                enabled: true,
                field_name: "_csrf".to_string(),
                header_name: "x-csrf-token".to_string(),
            },
        }
    }

    #[test]
    fn database_session_hosts_share_scoped_backend_by_default() {
        let services = services(SessionStoreTopology::Database);
        let mut left = BrowserHost::new_with_scope(
            "browser-db-shared".to_string(),
            services.clone(),
            "browser-db-shared",
        )
        .unwrap();
        let right = BrowserHost::new_with_scope(
            "browser-db-shared".to_string(),
            services,
            "browser-db-shared",
        )
        .unwrap();

        let issued = left
            .issue_session(
                SessionIssueRequest::new()
                    .for_principal("member-db")
                    .unwrap(),
                b"01234567012345670123456701234567",
                BrowserInstant::from_unix_seconds(100),
            )
            .unwrap();

        assert_eq!(left.session_store_kind(), SessionStoreBackendKind::Database);
        assert!(left.session_store_is_shared());
        assert_eq!(
            right
                .session(&issued.record.session_id)
                .and_then(|record| record.principal_id),
            Some("member-db".to_string())
        );
    }

    #[test]
    fn database_session_hosts_share_backend_when_reusing_an_explicit_client() {
        let services = services(SessionStoreTopology::Database);
        let client =
            DistributedSessionStoreClient::local_for_testing(SessionStoreBackendKind::Database);
        let mut left = BrowserHost::with_session_store_client(
            "browser-db-shared".to_string(),
            services.clone(),
            client.clone(),
        )
        .unwrap();
        let right = BrowserHost::with_session_store_client(
            "browser-db-shared".to_string(),
            services,
            client,
        )
        .unwrap();

        let issued = left
            .issue_session(
                SessionIssueRequest::new()
                    .for_principal("member-db")
                    .unwrap(),
                b"01234567012345670123456701234567",
                BrowserInstant::from_unix_seconds(100),
            )
            .unwrap();

        assert_eq!(left.session_store_kind(), SessionStoreBackendKind::Database);
        assert!(left.session_store_is_shared());
        assert_eq!(
            right
                .session(&issued.record.session_id)
                .and_then(|record| record.principal_id),
            Some("member-db".to_string())
        );
    }

    #[test]
    fn database_session_hosts_share_persistent_backend_across_independent_clients() {
        let services = services(SessionStoreTopology::Database);
        let namespace = persistent_namespace("browser-db-persistent");
        let mut left = BrowserHost::with_session_store_client(
            "browser-db-persistent".to_string(),
            services.clone(),
            DistributedSessionStoreClient::new(
                SessionStoreBackendKind::Database,
                shared::persistent_runtime(SessionStoreBackendKind::Database, namespace.clone()),
            ),
        )
        .unwrap();
        let right = BrowserHost::with_session_store_client(
            "browser-db-persistent".to_string(),
            services,
            DistributedSessionStoreClient::new(
                SessionStoreBackendKind::Database,
                shared::persistent_runtime(SessionStoreBackendKind::Database, namespace),
            ),
        )
        .unwrap();

        let issued = left
            .issue_session(
                SessionIssueRequest::new()
                    .for_principal("member-db")
                    .unwrap(),
                b"01234567012345670123456701234567",
                BrowserInstant::from_unix_seconds(100),
            )
            .unwrap();

        assert!(left.session_store_is_shared());
        assert_eq!(
            right
                .session(&issued.record.session_id)
                .and_then(|record| record.principal_id),
            Some("member-db".to_string())
        );
    }

    #[test]
    fn live_browser_rejects_memory_session_stores() {
        let services = services(SessionStoreTopology::Memory);
        let error =
            BrowserHost::new_with_scope("browser-memory".to_string(), services, "browser-memory")
                .unwrap_err();

        assert_eq!(
            error,
            BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost
        );
    }

    fn persistent_namespace(prefix: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!(
            "{prefix}-{}-{timestamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
    }
}
