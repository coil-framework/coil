use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PlatformConfig {
    pub app: AppConfig,
    pub server: ServerConfig,
    pub tls: TlsConfig,
    pub storage: StorageConfig,
    pub cache: CacheConfig,
    pub i18n: I18nConfig,
    pub seo: SeoConfig,
    pub auth: AuthConfig,
    pub modules: ModulesConfig,
    pub wasm: WasmConfig,
    pub jobs: JobsConfig,
    pub observability: ObservabilityConfig,
    pub assets: AssetsConfig,
}

impl Default for PlatformConfig {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            server: ServerConfig::default(),
            tls: TlsConfig::default(),
            storage: StorageConfig::default(),
            cache: CacheConfig::default(),
            i18n: I18nConfig::default(),
            seo: SeoConfig::default(),
            auth: AuthConfig::default(),
            modules: ModulesConfig::default(),
            wasm: WasmConfig::default(),
            jobs: JobsConfig::default(),
            observability: ObservabilityConfig::default(),
            assets: AssetsConfig::default(),
        }
    }
}

impl PlatformConfig {
    pub fn from_toml_str(source: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(source)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.app.name.trim().is_empty() {
            return Err(ConfigError::Validation("app.name must not be empty".into()));
        }

        if !self
            .i18n
            .supported_locales
            .iter()
            .any(|locale| locale == &self.i18n.default_locale)
        {
            return Err(ConfigError::Validation(
                "i18n.default_locale must be listed in i18n.supported_locales".into(),
            ));
        }

        if !self
            .i18n
            .supported_locales
            .iter()
            .any(|locale| locale == &self.i18n.fallback_locale)
        {
            return Err(ConfigError::Validation(
                "i18n.fallback_locale must be listed in i18n.supported_locales".into(),
            ));
        }

        if matches!(
            self.cache.l1,
            L1CacheBackend::Redis | L1CacheBackend::Valkey
        ) {
            return Err(ConfigError::Validation(
                "cache.l1 must use moka in the current platform design".into(),
            ));
        }

        if matches!(self.cache.l2, Some(DistributedCacheBackend::Moka)) {
            return Err(ConfigError::Validation(
                "cache.l2 must use redis or valkey when configured".into(),
            ));
        }

        match self.tls.mode {
            TlsMode::Acme => {
                if self.tls.challenge.is_none() || self.tls.provider.is_none() {
                    return Err(ConfigError::Validation(
                        "tls.mode=acme requires tls.challenge and tls.provider".into(),
                    ));
                }
            }
            TlsMode::CloudflareOriginCa => {
                if self.tls.provider != Some(TlsProvider::CloudflareOriginCa) {
                    return Err(ConfigError::Validation(
                        "tls.mode=cloudflare-origin-ca requires tls.provider=cloudflare-origin-ca"
                            .into(),
                    ));
                }
            }
            TlsMode::Manual | TlsMode::ExternalTermination | TlsMode::Disabled => {}
        }

        if self.assets.publish_manifest && self.assets.cdn_base_url.is_none() {
            return Err(ConfigError::Validation(
                "assets.publish_manifest=true requires assets.cdn_base_url".into(),
            ));
        }

        let mut seen = BTreeSet::new();
        for module in &self.modules.enabled {
            if !seen.insert(module) {
                return Err(ConfigError::Validation(format!(
                    "modules.enabled contains duplicate entry `{module}`"
                )));
            }
        }

        for module_key in self.modules.settings.keys() {
            if !self
                .modules
                .enabled
                .iter()
                .any(|enabled| enabled == module_key)
            {
                return Err(ConfigError::Validation(format!(
                    "modules.settings contains `{module_key}` but that module is not enabled"
                )));
            }
        }

        Ok(())
    }

    pub fn redacted_toml(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PlatformConfigPatch {
    pub app: Option<AppConfig>,
    pub server: Option<ServerConfig>,
    pub tls: Option<TlsConfig>,
    pub storage: Option<StorageConfig>,
    pub cache: Option<CacheConfig>,
    pub i18n: Option<I18nConfig>,
    pub seo: Option<SeoConfig>,
    pub auth: Option<AuthConfig>,
    pub modules: Option<ModulesConfig>,
    pub wasm: Option<WasmConfig>,
    pub jobs: Option<JobsConfig>,
    pub observability: Option<ObservabilityConfig>,
    pub assets: Option<AssetsConfig>,
}

pub struct PlatformConfigLoader {
    config: PlatformConfig,
}

impl Default for PlatformConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformConfigLoader {
    pub fn new() -> Self {
        Self {
            config: PlatformConfig::default(),
        }
    }

    pub fn with_defaults(defaults: PlatformConfig) -> Self {
        Self { config: defaults }
    }

    pub fn apply_patch(&mut self, patch: PlatformConfigPatch) -> &mut Self {
        if let Some(app) = patch.app {
            self.config.app = app;
        }
        if let Some(server) = patch.server {
            self.config.server = server;
        }
        if let Some(tls) = patch.tls {
            self.config.tls = tls;
        }
        if let Some(storage) = patch.storage {
            self.config.storage = storage;
        }
        if let Some(cache) = patch.cache {
            self.config.cache = cache;
        }
        if let Some(i18n) = patch.i18n {
            self.config.i18n = i18n;
        }
        if let Some(seo) = patch.seo {
            self.config.seo = seo;
        }
        if let Some(auth) = patch.auth {
            self.config.auth = auth;
        }
        if let Some(modules) = patch.modules {
            self.config.modules = modules;
        }
        if let Some(wasm) = patch.wasm {
            self.config.wasm = wasm;
        }
        if let Some(jobs) = patch.jobs {
            self.config.jobs = jobs;
        }
        if let Some(observability) = patch.observability {
            self.config.observability = observability;
        }
        if let Some(assets) = patch.assets {
            self.config.assets = assets;
        }

        self
    }

    pub fn apply_toml_patch_str(&mut self, source: &str) -> Result<&mut Self, ConfigError> {
        let patch: PlatformConfigPatch = toml::from_str(source)?;
        Ok(self.apply_patch(patch))
    }

    pub fn apply_toml_patch_env(&mut self, var_name: &str) -> Result<&mut Self, ConfigError> {
        match std::env::var(var_name) {
            Ok(source) => self.apply_toml_patch_str(&source),
            Err(std::env::VarError::NotPresent) => Ok(self),
            Err(std::env::VarError::NotUnicode(_)) => Err(ConfigError::Validation(format!(
                "environment variable `{var_name}` must be valid Unicode TOML"
            ))),
        }
    }

    pub fn build(self) -> Result<PlatformConfig, ConfigError> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Environment {
    Development,
    Test,
    Staging,
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Development
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AppConfig {
    pub name: String,
    pub environment: Environment,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            name: "davenda-app".into(),
            environment: Environment::Development,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ServerConfig {
    pub bind: String,
    pub trusted_proxies: Vec<String>,
    pub body_limit_bytes: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: "127.0.0.1:8080".into(),
            trusted_proxies: Vec::new(),
            body_limit_bytes: 10 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsMode {
    Disabled,
    Acme,
    CloudflareOriginCa,
    Manual,
    ExternalTermination,
}

impl Default for TlsMode {
    fn default() -> Self {
        Self::Disabled
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TlsChallenge {
    #[serde(rename = "http-01")]
    Http01,
    #[serde(rename = "tls-alpn-01")]
    TlsAlpn01,
    #[serde(rename = "dns-01")]
    Dns01,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsProvider {
    LetsEncrypt,
    CloudflareDns,
    CloudflareOriginCa,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", tag = "source")]
pub enum SecretRef {
    Env { name: String },
    Provider { provider: String, key: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct TlsConfig {
    pub mode: TlsMode,
    pub challenge: Option<TlsChallenge>,
    pub provider: Option<TlsProvider>,
    pub credentials: Option<SecretRef>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            mode: TlsMode::Disabled,
            challenge: None,
            provider: None,
            credentials: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageClass {
    PublicAsset,
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
}

impl Default for StorageClass {
    fn default() -> Self {
        Self::PublicUpload
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ObjectStoreKind {
    S3,
    S3Compatible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct StorageRule {
    pub path_prefix: String,
    pub class: StorageClass,
}

impl Default for StorageRule {
    fn default() -> Self {
        Self {
            path_prefix: "/".into(),
            class: StorageClass::PublicUpload,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct StorageConfig {
    pub default_class: StorageClass,
    pub object_store: Option<ObjectStoreKind>,
    pub local_root: PathBuf,
    pub path_defaults: Vec<StorageRule>,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            default_class: StorageClass::PublicUpload,
            object_store: Some(ObjectStoreKind::S3Compatible),
            local_root: PathBuf::from("/var/lib/davenda"),
            path_defaults: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum L1CacheBackend {
    Moka,
    Redis,
    Valkey,
}

impl Default for L1CacheBackend {
    fn default() -> Self {
        Self::Moka
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DistributedCacheBackend {
    Moka,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CacheConfig {
    pub l1: L1CacheBackend,
    pub l2: Option<DistributedCacheBackend>,
    pub namespace: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            l1: L1CacheBackend::Moka,
            l2: Some(DistributedCacheBackend::Redis),
            namespace: "davenda".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RouteLocalizationPolicy {
    Prefix,
    Domain,
    None,
}

impl Default for RouteLocalizationPolicy {
    fn default() -> Self {
        Self::Prefix
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct I18nConfig {
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub fallback_locale: String,
    pub route_strategy: RouteLocalizationPolicy,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            default_locale: "en-GB".into(),
            supported_locales: vec!["en-GB".into()],
            fallback_locale: "en-GB".into(),
            route_strategy: RouteLocalizationPolicy::Prefix,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SeoConfig {
    pub canonical_host: Option<String>,
    pub emit_json_ld: bool,
    pub emit_sitemaps: bool,
}

impl Default for SeoConfig {
    fn default() -> Self {
        Self {
            canonical_host: None,
            emit_json_ld: true,
            emit_sitemaps: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AuthConfig {
    pub package: String,
    pub explain_api: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            package: "platform-default-auth".into(),
            explain_api: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModulesConfig {
    pub enabled: Vec<String>,
    pub settings: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct WasmConfig {
    pub directory: PathBuf,
    pub default_time_limit_ms: u64,
    pub allow_network: bool,
    pub allowed_host_capabilities: Vec<String>,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            directory: PathBuf::from("extensions"),
            default_time_limit_ms: 50,
            allow_network: false,
            allowed_host_capabilities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum JobsBackend {
    InMemory,
    Redis,
    Valkey,
}

impl Default for JobsBackend {
    fn default() -> Self {
        Self::InMemory
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct JobsConfig {
    pub backend: JobsBackend,
    pub worker_concurrency: u16,
    pub scheduler_enabled: bool,
}

impl Default for JobsConfig {
    fn default() -> Self {
        Self {
            backend: JobsBackend::InMemory,
            worker_concurrency: 4,
            scheduler_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub logs: bool,
    pub metrics: bool,
    pub tracing: bool,
    pub health_endpoint: bool,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            logs: true,
            metrics: true,
            tracing: true,
            health_endpoint: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct AssetsConfig {
    pub publish_manifest: bool,
    pub cdn_base_url: Option<String>,
}

impl Default for AssetsConfig {
    fn default() -> Self {
        Self {
            publish_manifest: false,
            cdn_base_url: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse TOML config: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("failed to serialize TOML config: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("config validation failed: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_reference_style_config() {
        let config = PlatformConfig::from_toml_str(
            r#"
                [app]
                name = "showcase-events"
                environment = "production"

                [server]
                bind = "0.0.0.0:8080"
                trusted_proxies = ["10.0.0.0/8"]

                [tls]
                mode = "acme"
                challenge = "dns-01"
                provider = "cloudflare-dns"

                [storage]
                default_class = "public_upload"
                object_store = "s3"
                local_root = "/var/lib/platform"

                [cache]
                l1 = "moka"
                l2 = "redis"

                [i18n]
                default_locale = "en-GB"
                supported_locales = ["en-GB", "fr-FR"]
                fallback_locale = "en-GB"

                [seo]
                canonical_host = "www.example.com"
                emit_json_ld = true

                [auth]
                package = "platform-default-auth"
                explain_api = false

                [modules]
                enabled = ["cms-pages", "events"]

                [modules.settings.events]
                send_reminders = true

                [wasm]
                directory = "extensions"
                default_time_limit_ms = 50
                allow_network = false

                [jobs]
                backend = "redis"

                [observability]
                metrics = true
                tracing = true

                [assets]
                publish_manifest = true
                cdn_base_url = "https://cdn.example.com"
            "#,
        )
        .unwrap();

        assert_eq!(config.app.name, "showcase-events");
        assert_eq!(config.cache.l2, Some(DistributedCacheBackend::Redis));
        assert!(config.modules.settings.contains_key("events"));
    }

    #[test]
    fn loader_applies_late_overrides_by_section() {
        let mut loader = PlatformConfigLoader::new();

        loader
            .apply_toml_patch_str(
                r#"
                    [app]
                    name = "base-app"

                    [modules]
                    enabled = ["cms-pages"]
                "#,
            )
            .unwrap()
            .apply_toml_patch_str(
                r#"
                    [app]
                    name = "customer-app"

                    [modules]
                    enabled = ["cms-pages", "events"]
                "#,
            )
            .unwrap();

        let config = loader.build().unwrap();
        assert_eq!(config.app.name, "customer-app");
        assert_eq!(config.modules.enabled, vec!["cms-pages", "events"]);
    }

    #[test]
    fn rejects_duplicate_modules_and_invalid_locale_defaults() {
        let duplicate = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["cms-pages".into(), "cms-pages".into()],
                settings: BTreeMap::new(),
            },
            ..PlatformConfig::default()
        };
        assert!(matches!(
            duplicate.validate(),
            Err(ConfigError::Validation(_))
        ));

        let bad_locale = PlatformConfig {
            i18n: I18nConfig {
                default_locale: "fr-FR".into(),
                supported_locales: vec!["en-GB".into()],
                fallback_locale: "en-GB".into(),
                route_strategy: RouteLocalizationPolicy::Prefix,
            },
            ..PlatformConfig::default()
        };
        assert!(matches!(
            bad_locale.validate(),
            Err(ConfigError::Validation(_))
        ));
    }

    #[test]
    fn rejects_invalid_tls_and_cache_combinations() {
        let invalid_tls = PlatformConfig {
            tls: TlsConfig {
                mode: TlsMode::Acme,
                challenge: None,
                provider: Some(TlsProvider::LetsEncrypt),
                credentials: None,
            },
            ..PlatformConfig::default()
        };
        assert!(matches!(
            invalid_tls.validate(),
            Err(ConfigError::Validation(_))
        ));

        let invalid_cache = PlatformConfig {
            cache: CacheConfig {
                l1: L1CacheBackend::Redis,
                l2: Some(DistributedCacheBackend::Redis),
                namespace: "davenda".into(),
            },
            ..PlatformConfig::default()
        };
        assert!(matches!(
            invalid_cache.validate(),
            Err(ConfigError::Validation(_))
        ));
    }

    #[test]
    fn loader_can_apply_env_patch_and_emit_redacted_toml() {
        let var_name = "DAVENDA_TEST_PATCH";
        unsafe {
            std::env::set_var(
                var_name,
                r#"
                    [app]
                    name = "env-app"

                    [auth]
                    package = "customer-auth"
                "#,
            );
        }

        let mut loader = PlatformConfigLoader::new();
        loader.apply_toml_patch_env(var_name).unwrap();
        let config = loader.build().unwrap();
        let rendered = config.redacted_toml().unwrap();

        assert_eq!(config.app.name, "env-app");
        assert!(rendered.contains("customer-auth"));

        unsafe {
            std::env::remove_var(var_name);
        }
    }
}
