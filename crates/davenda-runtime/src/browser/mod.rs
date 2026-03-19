use std::time::Duration;

use super::*;
use davenda_core::BrowserSecurityError;

mod flash;
mod host;
mod session;
mod shared;
mod support;
#[cfg(test)]
mod testing;

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
