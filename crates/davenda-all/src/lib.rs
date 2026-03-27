#![forbid(unsafe_code)]

use davenda_admin::AdminModule;
use davenda_cms::CmsModule;
use davenda_commerce::{CommerceModule, CommercePaymentsStripeModule};
use davenda_events::EventsModule;
use davenda_media::MediaModule;
use davenda_memberships::MembershipsModule;
use davenda_ops::OpsModule;
use thiserror::Error;

pub use davenda_admin as admin;
pub use davenda_app as app;
pub use davenda_app::{CustomerAppComposition, CustomerAppManifest, CustomerAppRuntimePlan};
pub use davenda_auth as auth;
pub use davenda_auth::load_auth_model_package_at;
pub use davenda_auth::{AuthModelPackage, DefaultAuthModelPackage};
pub use davenda_cms as cms;
pub use davenda_commerce as commerce;
pub use davenda_config as config;
pub use davenda_config::{Environment, PlatformConfig};
pub use davenda_core::PlatformModule;
pub use davenda_customer_sdk as customer_sdk;
pub use davenda_customer_sdk::*;
pub use davenda_events as events;
pub use davenda_media as media;
pub use davenda_memberships as memberships;
pub use davenda_ops as ops;
pub use davenda_runtime as runtime;
pub use davenda_runtime::{
    EnvironmentSecretResolver, HttpServerHost, RuntimeBuildError, RuntimeBuilder, RuntimePlan,
    SecretResolver,
};

#[derive(Debug, Error)]
pub enum DavendaAllError {
    #[error("unsupported official module `{module}`")]
    UnsupportedOfficialModule { module: String },
}

pub const OFFICIAL_MODULE_NAMES: &[&str] = &[
    "admin",
    "cms",
    "commerce",
    "commerce-payments-stripe",
    "events",
    "media",
    "memberships",
    "ops",
];

pub mod modules {
    use super::*;

    pub fn admin() -> AdminModule {
        AdminModule::new()
    }

    pub fn cms() -> CmsModule {
        CmsModule::new()
    }

    pub fn commerce() -> CommerceModule {
        CommerceModule::new()
    }

    pub fn commerce_payments_stripe() -> CommercePaymentsStripeModule {
        CommercePaymentsStripeModule::new()
    }

    pub fn events() -> EventsModule {
        EventsModule::new()
    }

    pub fn media() -> MediaModule {
        MediaModule::new()
    }

    pub fn memberships() -> MembershipsModule {
        MembershipsModule::new()
    }

    pub fn ops() -> OpsModule {
        OpsModule::new()
    }
}

pub fn builder<P>(config: PlatformConfig, auth_package: P) -> RuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    with_official_modules(RuntimeBuilder::new(config, auth_package))
}

pub fn default_builder(config: PlatformConfig) -> RuntimeBuilder<DefaultAuthModelPackage> {
    builder(config, DefaultAuthModelPackage::default())
}

pub fn with_official_modules<P>(builder: RuntimeBuilder<P>) -> RuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    builder
        .with_module(modules::admin())
        .with_module(modules::cms())
        .with_module(modules::commerce())
        .with_module(modules::commerce_payments_stripe())
        .with_module(modules::events())
        .with_module(modules::media())
        .with_module(modules::memberships())
        .with_module(modules::ops())
}

pub trait RuntimeBuilderOfficialModulesExt<P> {
    fn with_official_modules(self) -> Self;
}

impl<P> RuntimeBuilderOfficialModulesExt<P> for RuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    fn with_official_modules(self) -> Self {
        with_official_modules(self)
    }
}

pub fn official_modules_from_config(
    config: &PlatformConfig,
) -> Result<Vec<Box<dyn PlatformModule>>, DavendaAllError> {
    official_modules_from_enabled(&config.modules.enabled)
}

pub fn official_modules_from_enabled(
    enabled: &[String],
) -> Result<Vec<Box<dyn PlatformModule>>, DavendaAllError> {
    let mut modules = Vec::with_capacity(enabled.len());
    for module in enabled {
        modules.push(official_module(module)?);
    }
    Ok(modules)
}

pub fn official_module(
    module: impl AsRef<str>,
) -> Result<Box<dyn PlatformModule>, DavendaAllError> {
    let module = module.as_ref();
    let boxed: Box<dyn PlatformModule> = match module {
        "admin" => Box::new(modules::admin()),
        "commerce" => Box::new(modules::commerce()),
        "commerce-payments-stripe" => Box::new(modules::commerce_payments_stripe()),
        "cms" => Box::new(modules::cms()),
        "events" => Box::new(modules::events()),
        "media" => Box::new(modules::media()),
        "memberships" => Box::new(modules::memberships()),
        "ops" => Box::new(modules::ops()),
        _ => {
            return Err(DavendaAllError::UnsupportedOfficialModule {
                module: module.to_string(),
            });
        }
    };
    Ok(boxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
[app]
name = "customer-root-smoke"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
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
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/tmp/davenda-all-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["admin", "cms", "commerce", "commerce-payments-stripe", "events", "media", "memberships", "ops"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"
max_attempts = 10

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#;

    #[test]
    fn builder_links_full_official_distribution() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();

        let plan = default_builder(config).build().unwrap();
        let names = plan
            .modules
            .iter()
            .map(|manifest| manifest.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, OFFICIAL_MODULE_NAMES);
    }

    #[test]
    fn module_helpers_expose_stable_customer_facing_factories() {
        assert_eq!(modules::admin().manifest().name, "admin");
        assert_eq!(modules::cms().manifest().name, "cms");
        assert_eq!(modules::commerce().manifest().name, "commerce");
        assert_eq!(
            modules::commerce_payments_stripe().manifest().name,
            "commerce-payments-stripe"
        );
        assert_eq!(modules::events().manifest().name, "events");
        assert_eq!(modules::media().manifest().name, "media");
        assert_eq!(modules::memberships().manifest().name, "memberships");
        assert_eq!(modules::ops().manifest().name, "ops");
    }

    #[test]
    fn official_module_reports_unknown_module_names() {
        let error = match official_module("not-real") {
            Ok(_) => panic!("expected unsupported module error"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "unsupported official module `not-real`");
    }
}
