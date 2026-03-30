use super::*;
use coil_core::BrowserSecurityError;

mod flash;
mod host;
mod live;
mod session;
mod support;
#[cfg(test)]
mod testing;
#[cfg(not(test))]
pub(crate) use live::live_shared_runtime;
#[cfg(test)]
pub(crate) use testing::test_only_sqlite_shared_runtime;

pub use flash::{FlashLevel, FlashMessage};
pub use host::{BrowserHost, BrowserHostBuildError, ResolvedBrowserRequest, RuntimeBrowserError};
pub use session::{
    BrowserInstant, BrowserSessionRecord, BrowserSessionStatus, DistributedSessionStoreClient,
    DistributedSessionStoreRuntime, IssuedBrowserSession, RotatedBrowserSession,
    SessionIssueRequest, SessionStoreBackendKind,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HttpMethod, RequestInput};
    use coil_core::{
        BrowserSecurityServices, CookiePolicy, CookieProtection, CsrfProtection,
        SessionSecurityServices, SessionStoreTopology,
    };
    use std::sync::Arc;
    use std::time::Duration;

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
                    same_site: coil_config::SameSitePolicy::Lax,
                    secure: true,
                    http_only: true,
                    protection: CookieProtection::Signed,
                },
                flash_cookie: CookiePolicy {
                    name: "flash".to_string(),
                    domain: None,
                    path: "/".to_string(),
                    same_site: coil_config::SameSitePolicy::Lax,
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
                .unwrap()
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
                .unwrap()
                .and_then(|record| record.principal_id),
            Some("member-db".to_string())
        );
    }

    #[test]
    fn database_session_hosts_share_explicit_backend_across_independent_clients() {
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

        assert!(left.session_store_is_shared());
        assert_eq!(
            right
                .session(&issued.record.session_id)
                .unwrap()
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

    #[test]
    fn live_browser_rejects_session_clients_without_explicit_shared_support() {
        #[derive(Debug)]
        struct UnconfiguredLiveSessionStoreRuntime;

        impl DistributedSessionStoreRuntime for UnconfiguredLiveSessionStoreRuntime {
            fn issue(&self, _record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError> {
                Ok(())
            }

            fn session(
                &self,
                _session_id: &str,
            ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
                Ok(None)
            }

            fn delete(&self, _session_id: &str) -> Result<(), RuntimeBrowserError> {
                Ok(())
            }

            fn revoke(
                &self,
                _session_id: &str,
                _now: BrowserInstant,
            ) -> Result<(), RuntimeBrowserError> {
                Ok(())
            }

            fn touch_active_session(
                &self,
                _session_id: &str,
                _idle_timeout: Duration,
                _now: BrowserInstant,
            ) -> Result<Option<String>, RuntimeBrowserError> {
                Ok(None)
            }

            fn is_shared_backend(&self) -> bool {
                false
            }
        }

        let services = services(SessionStoreTopology::Database);
        let client = DistributedSessionStoreClient::new(
            SessionStoreBackendKind::Database,
            Arc::new(UnconfiguredLiveSessionStoreRuntime),
        );

        let error =
            BrowserHost::with_session_store_client("browser-live".to_string(), services, client)
                .unwrap_err();

        assert_eq!(
            error,
            BrowserHostBuildError::LiveSharedSessionStoreRequiresExplicitRuntime {
                kind: SessionStoreBackendKind::Database,
            }
        );
    }

    #[test]
    fn live_browser_session_client_returns_typed_runtime_errors() {
        #[derive(Debug)]
        struct RejectedLiveSessionStoreRuntime;

        impl DistributedSessionStoreRuntime for RejectedLiveSessionStoreRuntime {
            fn issue(&self, _record: BrowserSessionRecord) -> Result<(), RuntimeBrowserError> {
                Err(RuntimeBrowserError::LiveSharedSessionStoreUnavailable {
                    kind: SessionStoreBackendKind::Database,
                    scope: "browser-live".to_string(),
                })
            }

            fn session(
                &self,
                _session_id: &str,
            ) -> Result<Option<BrowserSessionRecord>, RuntimeBrowserError> {
                Err(RuntimeBrowserError::LiveSharedSessionStoreUnavailable {
                    kind: SessionStoreBackendKind::Database,
                    scope: "browser-live".to_string(),
                })
            }

            fn delete(&self, _session_id: &str) -> Result<(), RuntimeBrowserError> {
                Err(RuntimeBrowserError::LiveSharedSessionStoreUnavailable {
                    kind: SessionStoreBackendKind::Database,
                    scope: "browser-live".to_string(),
                })
            }

            fn revoke(
                &self,
                _session_id: &str,
                _now: BrowserInstant,
            ) -> Result<(), RuntimeBrowserError> {
                Err(RuntimeBrowserError::LiveSharedSessionStoreUnavailable {
                    kind: SessionStoreBackendKind::Database,
                    scope: "browser-live".to_string(),
                })
            }

            fn touch_active_session(
                &self,
                _session_id: &str,
                _idle_timeout: Duration,
                _now: BrowserInstant,
            ) -> Result<Option<String>, RuntimeBrowserError> {
                Err(RuntimeBrowserError::LiveSharedSessionStoreUnavailable {
                    kind: SessionStoreBackendKind::Database,
                    scope: "browser-live".to_string(),
                })
            }

            fn is_shared_backend(&self) -> bool {
                false
            }
        }

        let client = DistributedSessionStoreClient::new(
            SessionStoreBackendKind::Database,
            Arc::new(RejectedLiveSessionStoreRuntime),
        );

        let error = client
            .issue(BrowserSessionRecord {
                session_id: "session-1".to_string(),
                principal_id: Some("member".to_string()),
                issued_at: BrowserInstant::from_unix_seconds(1),
                last_seen_at: BrowserInstant::from_unix_seconds(1),
                idle_expires_at: BrowserInstant::from_unix_seconds(60),
                absolute_expires_at: BrowserInstant::from_unix_seconds(120),
                revoked_at: None,
            })
            .unwrap_err();

        assert_eq!(
            error,
            RuntimeBrowserError::LiveSharedSessionStoreUnavailable {
                kind: SessionStoreBackendKind::Database,
                scope: "browser-live".to_string(),
            }
        );
    }

    #[test]
    fn resolve_request_reissues_an_anonymous_session_when_cookie_state_is_missing() {
        let services = services(SessionStoreTopology::Database);
        let client =
            DistributedSessionStoreClient::local_for_testing(SessionStoreBackendKind::Database);
        let mut issuer = BrowserHost::with_session_store_client(
            "browser-db-shared".to_string(),
            services.clone(),
            client.clone(),
        )
        .unwrap();
        let mut resolver = BrowserHost::with_session_store_client(
            "browser-db-shared".to_string(),
            services,
            client,
        )
        .unwrap();
        let cookie_secret = b"01234567012345670123456701234567";
        let now = BrowserInstant::from_unix_seconds(100);
        let issued = issuer
            .issue_session(
                SessionIssueRequest::new()
                    .for_principal("member-db")
                    .unwrap(),
                cookie_secret,
                now,
            )
            .unwrap();

        issuer.sessions.delete(&issued.record.session_id).unwrap();

        let request = RequestInput::new(HttpMethod::Get, "www.example.com", "/")
            .unwrap()
            .with_session_cookie(issued.cookie_value);
        let resolved = resolver
            .resolve_request(&request, cookie_secret, now)
            .unwrap();

        assert!(resolved.session.resolved_from_cookie);
        assert!(resolved.session.session_id.is_some());
        assert_ne!(
            resolved.session.session_id.as_deref(),
            Some(issued.record.session_id.as_str())
        );
        assert_eq!(resolved.principal_id, None);
        assert_eq!(resolved.response_cookies.len(), 1);
        assert!(resolved.response_cookies[0].contains("session="));
    }

    #[test]
    fn resolve_request_rejects_expired_cookie_backed_sessions() {
        let services = services(SessionStoreTopology::Database);
        let client =
            DistributedSessionStoreClient::local_for_testing(SessionStoreBackendKind::Database);
        let mut host = BrowserHost::with_session_store_client(
            "browser-db-shared".to_string(),
            services,
            client,
        )
        .unwrap();
        let cookie_secret = b"01234567012345670123456701234567";
        let issued = host
            .issue_session(
                SessionIssueRequest::new()
                    .for_principal("member-db")
                    .unwrap(),
                cookie_secret,
                BrowserInstant::from_unix_seconds(100),
            )
            .unwrap();

        let request = RequestInput::new(HttpMethod::Get, "www.example.com", "/")
            .unwrap()
            .with_session_cookie(issued.cookie_value);
        let error = host
            .resolve_request(
                &request,
                cookie_secret,
                BrowserInstant::from_unix_seconds(4_000),
            )
            .unwrap_err();

        assert_eq!(
            error,
            RuntimeBrowserError::ExpiredSession {
                session_id: issued.record.session_id,
            }
        );
    }
}
