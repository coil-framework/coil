use async_trait::async_trait;
use davenda_auth::{
    configured_auth_model_package_selection, CapabilityExplanation, LiveAuthExplainHost,
    LiveAuthExplainRequest,
};
use davenda_config::PlatformConfig;
use std::sync::Arc;

use crate::cli::args::AuthExplainInvocation;
use crate::cli::error::CliRunError;

#[async_trait]
pub(crate) trait AuthExplainBackend: Send + Sync {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError>;
}

#[derive(Debug, Clone)]
pub(crate) struct LiveAuthExplainBackend {
    explainer: Arc<LiveAuthExplainHost>,
}

impl LiveAuthExplainBackend {
    pub(crate) fn from_config(config: &PlatformConfig) -> Result<Self, CliRunError> {
        let package = resolve_configured_auth_package_selection(config);
        let explainer = LiveAuthExplainHost::from_config(config, package).map_err(|error| {
            CliRunError::execution(format!(
                "failed to initialize the live auth explain backend: {error}"
            ))
        })?;

        Ok(Self {
            explainer: Arc::new(explainer),
        })
    }
}

#[async_trait]
impl AuthExplainBackend for LiveAuthExplainBackend {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError> {
        let request = LiveAuthExplainRequest {
            subject: invocation.subject.clone(),
            capability: invocation.capability,
            object: invocation.resource.clone(),
            options: invocation.options,
        };

        self.explainer
            .explain_capability(&request)
            .await
            .map_err(|error| {
                CliRunError::execution(format!("failed to build the auth explanation: {error}"))
            })
    }
}

fn resolve_configured_auth_package_selection(
    config: &PlatformConfig,
) -> davenda_auth::AuthModelPackageSelection {
    // The CLI explain path is keyed by the deployment-configured auth package identity.
    // This keeps the live backend aligned with replacement packages instead of assuming
    // the default package name is the only valid deployment configuration.
    configured_auth_model_package_selection(config.auth.package.clone())
}

#[cfg(test)]
use std::sync::Mutex;

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct StaticAuthExplainBackend {
    response: CapabilityExplanation,
    requests: Arc<Mutex<Vec<AuthExplainInvocation>>>,
}

#[cfg(test)]
impl StaticAuthExplainBackend {
    pub(crate) fn new(response: CapabilityExplanation) -> Self {
        Self {
            response,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Vec<AuthExplainInvocation> {
        self.requests
            .lock()
            .expect("static auth explain backend mutex poisoned")
            .clone()
    }
}

#[cfg(test)]
#[async_trait]
impl AuthExplainBackend for StaticAuthExplainBackend {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError> {
        self.requests
            .lock()
            .expect("static auth explain backend mutex poisoned")
            .push(invocation.clone());
        Ok(self.response.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::AuthExplainInvocation;
    use davenda_auth::{
        AllowedExplanation, AuthModelPackage, Capability, DefaultAuthModelPackage, DefaultSubject,
        Entity, ExplainDecision, ExplainOptions, ExplainStep, ExplainTrace,
    };
    use davenda_config::PlatformConfig;
    use std::path::PathBuf;

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
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
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
local_root = "/tmp/davenda-cli-live"

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
package = "platform-default-auth"
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

    fn config(explain_api: bool) -> PlatformConfig {
        let mut config = PlatformConfig::from_toml_str(BASE_CONFIG).unwrap();
        config.auth.explain_api = explain_api;
        config.database.url = None;
        config
    }

    fn invocation() -> AuthExplainInvocation {
        AuthExplainInvocation {
            config_path: PathBuf::from("/tmp/platform.toml"),
            subject: DefaultSubject::entity(Entity::user("alice")),
            capability: Capability::CmsPageRead,
            resource: Entity::page("homepage"),
            options: ExplainOptions::default(),
        }
    }

    fn explanation() -> davenda_auth::CapabilityExplanation {
        let package = DefaultAuthModelPackage::default();
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let capability = Capability::CmsPageRead;
        let resource = Entity::page("homepage");
        davenda_auth::CapabilityExplanation {
            manifest: package.manifest().clone(),
            subject: subject.clone(),
            capability,
            object: resource.clone(),
            binding: package.binding_for(capability).unwrap().clone(),
            decision: ExplainDecision::Allow,
            options: ExplainOptions::default(),
            trace: ExplainTrace::Allowed(AllowedExplanation {
                steps: vec![ExplainStep::Start {
                    node: davenda_auth::ExplainedNode {
                        object: resource,
                        relation: None,
                    },
                }],
            }),
        }
    }

    #[test]
    fn from_config_rejects_disabled_deployment_config() {
        let error = LiveAuthExplainBackend::from_config(&config(false)).unwrap_err();
        assert!(error.to_string().contains("disabled by deployment config"));
    }

    #[test]
    fn from_config_accepts_replacement_package_identity() {
        let mut config = config(true);
        config.auth.package = "platform-extended-auth".to_string();

        let package = resolve_configured_auth_package_selection(&config);

        assert_eq!(package.manifest().name, "platform-extended-auth");
        assert_ne!(
            package.manifest().name,
            DefaultAuthModelPackage::default().manifest().name
        );
        assert_eq!(
            package
                .package()
                .binding_for(Capability::CmsPageRead)
                .unwrap(),
            DefaultAuthModelPackage::default()
                .binding_for(Capability::CmsPageRead)
                .unwrap()
        );

        let backend = LiveAuthExplainBackend::from_config(&config).unwrap();
        assert!(format!("{backend:?}").contains("platform-extended-auth"));
    }

    #[test]
    fn explain_uses_the_live_backend_and_reports_a_live_backend_failure() {
        let backend = LiveAuthExplainBackend::from_config(&config(true)).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let error = runtime
            .block_on(async { backend.explain(&invocation()).await })
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("failed to build the auth explanation"));
        assert!(error.to_string().contains("live auth backend"));
    }

    #[test]
    fn static_backend_still_supports_cli_render_tests() {
        let backend = StaticAuthExplainBackend::new(explanation());
        let request = invocation();

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime
            .block_on(async { backend.explain(&request).await })
            .unwrap();

        assert_eq!(result.decision, ExplainDecision::Allow);
        assert_eq!(backend.requests(), vec![request]);
    }
}
