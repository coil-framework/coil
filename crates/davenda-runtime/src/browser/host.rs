use super::flash::{FlashMessage, deserialize_flash_messages, serialize_flash_messages};
use super::session::{
    BrowserInstant, BrowserSessionRecord, BrowserSessionStatus, DistributedSessionStoreClient,
    IssuedBrowserSession, RotatedBrowserSession, SessionIssueRequest, SessionStoreBackend,
    SessionStoreBackendKind, issue_session,
};
use super::support::{
    FLASH_COOKIE_MAX_AGE_SECS, map_flash_cookie_error, map_session_cookie_error,
    validate_browser_value,
};
use super::*;
use std::time::Duration;
use thiserror::Error;

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
    #[error(
        "live browser session store `{kind:?}` for `{scope}` requires an explicit distributed runtime"
    )]
    LiveSharedSessionStoreUnavailable {
        kind: SessionStoreBackendKind,
        scope: String,
    },
    #[error("live browser session store `{kind:?}` for `{scope}` failed: {reason}")]
    LiveSharedSessionStoreFailure {
        kind: SessionStoreBackendKind,
        scope: String,
        reason: String,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BrowserHostBuildError {
    #[error("memory session stores are test-only and cannot back a live browser host")]
    MemoryStoreRequiresTestOnlyBrowserHost,
    #[error("memory session stores cannot use a distributed session client")]
    MemoryStoreCannotUseDistributedClient,
    #[error(
        "live browser session stores require an explicit distributed runtime; `{kind:?}` is not live-supported"
    )]
    LiveSharedSessionStoreRequiresExplicitRuntime { kind: SessionStoreBackendKind },
    #[error(
        "live browser session store `{kind:?}` for `{scope}` could not be initialized at `{path}`: {reason}"
    )]
    LiveSharedSessionStoreInitializationFailed {
        kind: SessionStoreBackendKind,
        scope: String,
        path: String,
        reason: String,
    },
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
    pub(super) session_store_kind: SessionStoreBackendKind,
    pub(super) sessions: SessionStoreBackend,
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

    pub fn with_session_store_client(
        customer_app: String,
        services: BrowserSecurityServices,
        client: DistributedSessionStoreClient,
    ) -> Result<Self, BrowserHostBuildError> {
        let (session_store_kind, sessions) =
            SessionStoreBackend::with_client(&services.sessions, client)?;
        if !sessions.is_live_shared_state_supported() {
            return Err(
                BrowserHostBuildError::LiveSharedSessionStoreRequiresExplicitRuntime {
                    kind: session_store_kind,
                },
            );
        }
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
        issue_session(self, request, cookie_secret, now)
    }

    pub fn rotate_session(
        &mut self,
        session_id: &str,
        cookie_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<RotatedBrowserSession, RuntimeBrowserError> {
        let session_id = validate_browser_value("session_id", session_id.to_string())?;
        let existing = self.sessions.session(&session_id)?.ok_or_else(|| {
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
                self.sessions.delete(&session_id)?;
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

    pub fn clear_session_cookie_header(&self) -> String {
        self.services
            .sessions
            .session_cookie
            .render_set_cookie("", Some(Duration::from_secs(0)))
    }

    pub fn session(
        &self,
        session_id: &str,
    ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
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

    pub(super) fn issue_cookie_for_record(
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
