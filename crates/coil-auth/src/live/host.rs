use super::{LiveAuthError, LiveAuthExplainRequest};
use crate::{AuthModelPackageSelection, CapabilityExplanation, CoilAuth};
use coil_config::PlatformConfig;
use coil_data::DataRuntime;
use std::fmt;
use std::sync::OnceLock;

#[cfg(test)]
use crate::{Capability, DefaultAuthModelPackage, DefaultSubject, Entity, ExplainOptions};

pub struct LiveAuthExplainHost {
    data: DataRuntime,
    tenant_id: i64,
    auth_package: AuthModelPackageSelection,
    explainer: OnceLock<Result<PostgresAuthExplainer, String>>,
}

impl fmt::Debug for LiveAuthExplainHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveAuthExplainHost")
            .field("tenant_id", &self.tenant_id)
            .field("auth_package", &self.auth_package.manifest().name)
            .finish_non_exhaustive()
    }
}

impl LiveAuthExplainHost {
    pub fn from_config(
        config: &PlatformConfig,
        auth_package: AuthModelPackageSelection,
    ) -> Result<Self, LiveAuthError> {
        if !config.auth.explain_api {
            return Err(LiveAuthError::ExplainApiDisabled);
        }

        let data = DataRuntime::from_config(&config.database).map_err(|error| {
            LiveAuthError::BackendInitialization {
                reason: error.to_string(),
            }
        })?;

        Self::from_runtime(config, data, auth_package)
    }

    pub fn from_runtime(
        config: &PlatformConfig,
        data: DataRuntime,
        auth_package: AuthModelPackageSelection,
    ) -> Result<Self, LiveAuthError> {
        if !config.auth.explain_api {
            return Err(LiveAuthError::ExplainApiDisabled);
        }

        Ok(Self {
            data,
            tenant_id: config.auth.tenant_id,
            auth_package,
            explainer: OnceLock::new(),
        })
    }

    fn explainer(&self) -> Result<&PostgresAuthExplainer, LiveAuthError> {
        match self.explainer.get_or_init(|| self.build_explainer()) {
            Ok(explainer) => Ok(explainer),
            Err(reason) => Err(LiveAuthError::BackendInitialization {
                reason: reason.clone(),
            }),
        }
    }

    fn build_explainer(&self) -> Result<PostgresAuthExplainer, String> {
        let client = self
            .data
            .clone()
            .connect_lazy_postgres()
            .map_err(|error| error.to_string())?;
        let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());

        Ok(PostgresAuthExplainer {
            auth: CoilAuth::new(engine, self.tenant_id),
            package: self.auth_package.clone(),
        })
    }

    pub async fn explain_capability(
        &self,
        request: &LiveAuthExplainRequest,
    ) -> Result<CapabilityExplanation, LiveAuthError> {
        self.explainer()?.explain_capability(request).await
    }
}

#[derive(Clone)]
struct PostgresAuthExplainer {
    auth: CoilAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: AuthModelPackageSelection,
}

impl PostgresAuthExplainer {
    async fn explain_capability(
        &self,
        request: &LiveAuthExplainRequest,
    ) -> Result<CapabilityExplanation, LiveAuthError> {
        self.auth
            .explain_capability_with_options(
                self.package.package(),
                &request.subject,
                request.capability,
                &request.object,
                request.options,
            )
            .await
            .map_err(|error| LiveAuthError::Explain {
                reason: error.to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthModelPackageSelection, configured_auth_model_package};
    use coil_config::PlatformConfig;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    const BASE_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = []

[http.session]
store = "memory"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "coil_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "coil_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
deployment = "single_node"
single_node_escape_hatch = "explicit_single_node"
local_root = "/tmp/coil-auth-live"

[cache]
l1 = "moka"

[i18n]
default_locale = "en"
supported_locales = ["en"]
fallback_locale = "en"
localized_routes = false

[seo]
canonical_host = "example.com"
emit_json_ld = true
sitemap_enabled = true

[auth]
package = "coil-default-auth"
explain_api = true
tenant_id = 42

[modules]
enabled = ["cms"]

[wasm]
directory = "wasm"
default_time_limit_ms = 1000
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#;

    fn explain_request() -> LiveAuthExplainRequest {
        LiveAuthExplainRequest {
            subject: DefaultSubject::entity(Entity::user("alice")),
            capability: Capability::CmsPageRead,
            object: Entity::page("homepage"),
            options: ExplainOptions::default(),
        }
    }

    fn config(explain_api: bool) -> PlatformConfig {
        let mut config = PlatformConfig::from_toml_str(BASE_CONFIG).unwrap();
        config.auth.explain_api = explain_api;
        config.database.url = None;
        config
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        fn raw_waker() -> RawWaker {
            fn clone(_: *const ()) -> RawWaker {
                raw_waker()
            }
            fn wake(_: *const ()) {}
            fn wake_by_ref(_: *const ()) {}
            fn drop(_: *const ()) {}
            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(clone, wake, wake_by_ref, drop),
            )
        }

        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(&waker);

        loop {
            match Future::poll(Pin::as_mut(&mut future), &mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn from_runtime_rejects_disabled_deployment_config() {
        let package = AuthModelPackageSelection::new(DefaultAuthModelPackage::default());
        let error = LiveAuthExplainHost::from_runtime(
            &config(false),
            DataRuntime::from_config(&config(false).database).unwrap(),
            package,
        )
        .unwrap_err();

        assert_eq!(error, LiveAuthError::ExplainApiDisabled);
    }

    #[test]
    fn explain_capability_uses_the_live_postgres_path() {
        let package = AuthModelPackageSelection::new(DefaultAuthModelPackage::default());
        let host = LiveAuthExplainHost::from_runtime(
            &config(true),
            DataRuntime::from_config(&config(true).database).unwrap(),
            package,
        )
        .unwrap();

        let error = block_on(host.explain_capability(&explain_request())).unwrap_err();

        assert!(matches!(error, LiveAuthError::BackendInitialization { .. }));
    }

    #[test]
    fn from_runtime_keeps_the_configured_package_identity() {
        let package =
            AuthModelPackageSelection::new(configured_auth_model_package("coil-extended-auth"));
        let host = LiveAuthExplainHost::from_runtime(
            &config(true),
            DataRuntime::from_config(&config(true).database).unwrap(),
            package,
        )
        .unwrap();

        assert!(format!("{host:?}").contains("coil-extended-auth"));
    }
}
