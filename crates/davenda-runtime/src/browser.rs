use std::collections::HashMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;
use davenda_core::{BrowserSecurityError, CookieSigner};

const FLASH_COOKIE_MAX_AGE_SECS: u64 = 300;
static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionStoreBackendKind {
    Local,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
    sessions: HashMap<String, BrowserSessionRecord>,
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

#[derive(Debug, Clone)]
enum SessionStoreBackend {
    Local(SessionStoreState),
    Shared(Arc<Mutex<SessionStoreState>>),
}

impl SessionStoreBackend {
    fn new(topology: davenda_core::SessionStoreTopology) -> (SessionStoreBackendKind, Self) {
        match topology {
            davenda_core::SessionStoreTopology::Memory => (
                SessionStoreBackendKind::Local,
                Self::Local(SessionStoreState::default()),
            ),
            davenda_core::SessionStoreTopology::Database => (
                SessionStoreBackendKind::Database,
                Self::Shared(Arc::new(Mutex::new(SessionStoreState::default()))),
            ),
            davenda_core::SessionStoreTopology::Redis => (
                SessionStoreBackendKind::Redis,
                Self::Shared(Arc::new(Mutex::new(SessionStoreState::default()))),
            ),
            davenda_core::SessionStoreTopology::Valkey => (
                SessionStoreBackendKind::Valkey,
                Self::Shared(Arc::new(Mutex::new(SessionStoreState::default()))),
            ),
        }
    }

    fn is_shared(&self) -> bool {
        matches!(self, Self::Shared(_))
    }

    fn with_state<R>(&self, f: impl FnOnce(&SessionStoreState) -> R) -> R {
        match self {
            Self::Local(state) => f(state),
            Self::Shared(state) => {
                let guard = state.lock().expect("session backend mutex poisoned");
                f(&guard)
            }
        }
    }

    fn with_state_mut<R>(&mut self, f: impl FnOnce(&mut SessionStoreState) -> R) -> R {
        match self {
            Self::Local(state) => f(state),
            Self::Shared(state) => {
                let mut guard = state.lock().expect("session backend mutex poisoned");
                f(&mut guard)
            }
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

#[derive(Debug, Clone)]
pub struct BrowserHost {
    pub customer_app: String,
    pub services: BrowserSecurityServices,
    session_store_kind: SessionStoreBackendKind,
    sessions: SessionStoreBackend,
}

impl BrowserHost {
    pub(crate) fn new(customer_app: String, services: BrowserSecurityServices) -> Self {
        let (session_store_kind, sessions) = SessionStoreBackend::new(services.sessions.store);
        Self {
            customer_app,
            services,
            session_store_kind,
            sessions,
        }
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
        self.sessions.with_state_mut(|state| state.issue(record));
        Ok(issued)
    }

    pub fn rotate_session(
        &mut self,
        session_id: &str,
        cookie_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<RotatedBrowserSession, RuntimeBrowserError> {
        let session_id = validate_browser_value("session_id", session_id.to_string())?;
        let existing = self.sessions.get_mut(&session_id).ok_or_else(|| {
            RuntimeBrowserError::UnknownSession {
                session_id: session_id.clone(),
            }
        })?;

        match existing.status_at(now) {
            BrowserSessionStatus::Active => {
                let principal_id = existing.principal_id.clone();
                existing.revoked_at = Some(now);
                let issued =
                    self.issue_session(SessionIssueRequest { principal_id }, cookie_secret, now)?;
                Ok(RotatedBrowserSession {
                    previous_session_id: session_id,
                    issued,
                })
            }
            BrowserSessionStatus::IdleExpired | BrowserSessionStatus::AbsoluteExpired => {
                Err(RuntimeBrowserError::ExpiredSession { session_id })
            }
            BrowserSessionStatus::Revoked => {
                Err(RuntimeBrowserError::RevokedSession { session_id })
            }
        }
    }

    pub fn revoke_session(
        &mut self,
        session_id: &str,
        now: BrowserInstant,
    ) -> Result<(), RuntimeBrowserError> {
        let session_id = validate_browser_value("session_id", session_id.to_string())?;
        self.sessions
            .with_state_mut(|state| state.revoke(&session_id, now))
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
        let value = CookieSigner::new(self.services.sessions.flash_cookie.clone())
            .sign(cookie_secret, &payload)
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
        self.sessions.with_state(|state| state.session(session_id))
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
        let cookie_value = CookieSigner::new(self.services.sessions.session_cookie.clone())
            .sign(cookie_secret, &record.session_id)
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
        CookieSigner::new(self.services.sessions.session_cookie.clone())
            .verify(cookie_secret, cookie)
            .map_err(map_session_cookie_error)
    }

    fn consume_flash_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<Vec<FlashMessage>, RuntimeBrowserError> {
        let payload = CookieSigner::new(self.services.sessions.flash_cookie.clone())
            .verify(cookie_secret, cookie)
            .map_err(map_flash_cookie_error)?;
        deserialize_flash_messages(&payload)
    }

    fn touch_active_session(
        &mut self,
        session_id: &str,
        cookie_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<(Option<String>, String), RuntimeBrowserError> {
        let principal_id = self.sessions.with_state_mut(|state| {
            state.touch_active_session(session_id, self.services.sessions.idle_timeout, now)
        })?;
        let cookie_value = CookieSigner::new(self.services.sessions.session_cookie.clone())
            .sign(cookie_secret, session_id)
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
