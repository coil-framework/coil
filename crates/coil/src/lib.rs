#![forbid(unsafe_code)]

use coil_admin::AdminModule;
use coil_cms::CmsModule;
use coil_commerce::{CommerceModule, CommercePaymentsStripeModule};
use coil_events::EventsModule;
use coil_media::MediaModule;
use coil_memberships::MembershipsModule;
use coil_ops::OpsModule;
use coil_runtime::{
    RuntimeBuilder, customer_root_bootstrap_inputs_from_env,
    customer_root_bootstrap_inputs_from_paths,
};
use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub use coil_admin as admin;
pub use coil_app as app;
pub use coil_app::CustomerAppManifest;
pub use coil_auth as auth;
pub use coil_auth::{AuthModelPackage, DefaultAuthModelPackage};
pub use coil_cms as cms;
pub use coil_commerce as commerce;
pub use coil_config as config;
pub use coil_config::{Environment, PlatformConfig};
pub use coil_core::PlatformModule;
pub use coil_customer_sdk as customer_sdk;
pub use coil_customer_sdk::*;
pub use coil_events as events;
pub use coil_media as media;
pub use coil_memberships as memberships;
pub use coil_ops as ops;

#[derive(Debug, Error)]
pub enum CoilError {
    #[error("unsupported official module `{module}`")]
    UnsupportedOfficialModule { module: String },
    #[error("failed to resolve the current working directory: {0}")]
    CurrentDirectory(std::io::Error),
    #[error("customer app manifest `{path}` could not be loaded: {reason}")]
    ManifestLoad { path: PathBuf, reason: String },
    #[error("platform config `{path}` could not be loaded: {reason}")]
    ConfigLoad { path: PathBuf, reason: String },
    #[error("customer runtime build failed: {reason}")]
    RuntimeBuild { reason: String },
    #[error("customer runtime bootstrap failed: {reason}")]
    Bootstrap { reason: String },
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

#[derive(Default)]
pub struct CoilBuilder {
    customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
}

pub fn builder() -> CoilBuilder {
    CoilBuilder::default()
}

impl CoilBuilder {
    pub fn with_customer_plugin<C>(mut self, plugin: C) -> Self
    where
        C: CustomerBackendPlugin,
    {
        self.customer_plugins.push(Box::new(plugin));
        self
    }

    pub fn run_from_env(self) -> Result<(), CoilError> {
        let app_root = env::current_dir().map_err(CoilError::CurrentDirectory)?;
        let bootstrap = customer_root_bootstrap_inputs_from_env()
            .map_err(|error| match error {
                coil_runtime::RuntimeBootstrapError::ConfigLoad { path, reason } => {
                    CoilError::ConfigLoad { path, reason }
                }
                coil_runtime::RuntimeBootstrapError::ConfigNotFound { app_root } => {
                    CoilError::ConfigLoad {
                        path: app_root.join("platform.toml"),
                        reason:
                            "set `COIL_CONFIG` or add `platform.toml` / `platform.dev.toml` to the customer app root"
                                .to_string(),
                    }
                }
                other => CoilError::RuntimeBuild {
                    reason: other.to_string(),
                },
            })?;
        self.run_from_paths(
            app_root,
            bootstrap.config_path,
            env::var("COIL_BIND").ok(),
        )
    }

    pub fn run_from_paths(
        self,
        app_root: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
        bind_override: Option<String>,
    ) -> Result<(), CoilError> {
        let app_root = app_root.as_ref();
        let manifest_path = app_root.join("app.toml");
        let manifest =
            coil_app::CustomerAppManifest::from_file(&manifest_path).map_err(|error| {
                CoilError::ManifestLoad {
                    path: manifest_path.clone(),
                    reason: error.to_string(),
                }
            })?;

        let bootstrap = customer_root_bootstrap_inputs_from_paths(
            app_root,
            config_path,
        )
        .map_err(|error| match error {
            coil_runtime::RuntimeBootstrapError::ConfigLoad { path, reason } => {
                CoilError::ConfigLoad { path, reason }
            }
            coil_runtime::RuntimeBootstrapError::ConfigNotFound { app_root } => {
                CoilError::ConfigLoad {
                    path: app_root.join("platform.toml"),
                    reason:
                        "set `COIL_CONFIG` or add `platform.toml` / `platform.dev.toml` to the customer app root"
                            .to_string(),
                }
            }
            other => CoilError::RuntimeBuild {
                reason: other.to_string(),
            },
        })?;
        let modules = official_modules_from_config(&bootstrap.config).map_err(|error| {
            CoilError::RuntimeBuild {
                reason: error.to_string(),
            }
        })?;
        let runtime_plan = manifest
            .build_customer_root_runtime_plan_with_extensions_and_customer_plugins_at(
                bootstrap.config,
                bootstrap.auth_package,
                modules,
                Vec::new(),
                self.customer_plugins,
                app_root,
            )
            .map_err(|error| CoilError::RuntimeBuild {
                reason: error.to_string(),
            })?;

        runtime_plan
            .runtime
            .serve_from_env(bind_override)
            .map_err(|error| CoilError::Bootstrap {
                reason: error.to_string(),
            })?;
        Ok(())
    }
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
) -> Result<Vec<Box<dyn PlatformModule>>, CoilError> {
    official_modules_from_enabled(&config.modules.enabled)
}

pub fn official_modules_from_enabled(
    enabled: &[String],
) -> Result<Vec<Box<dyn PlatformModule>>, CoilError> {
    let mut modules = Vec::with_capacity(enabled.len());
    for module in enabled {
        modules.push(official_module(module)?);
    }
    Ok(modules)
}

pub fn official_module(
    module: impl AsRef<str>,
) -> Result<Box<dyn PlatformModule>, CoilError> {
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
            return Err(CoilError::UnsupportedOfficialModule {
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
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/tmp/coil-tests"
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
package = "coil-default-auth"
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

        let plan = with_official_modules(coil_runtime::RuntimeBuilder::new(
            config,
            DefaultAuthModelPackage::default(),
        ))
        .build()
        .unwrap();
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
