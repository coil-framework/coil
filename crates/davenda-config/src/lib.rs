use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

impl PlatformConfig {
    pub fn from_toml_str(input: &str) -> Result<Self, ConfigError> {
        Self::from_toml_str_with_overlays(input, std::iter::empty::<&str>())
    }

    pub fn from_toml_str_with_overlays<'a>(
        input: &str,
        overlays: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ConfigError> {
        let mut merged: toml::Value = toml::from_str(input)?;

        for overlay in overlays {
            let overlay_value: toml::Value = toml::from_str(overlay)?;
            merge_toml_value(&mut merged, overlay_value);
        }

        let config: Self = merged.try_into()?;
        config.validate()?;
        Ok(config)
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(ConfigError::Io)?;
        Self::from_toml_str(&contents)
    }

    pub fn from_toml_str_with_env_overlays(
        input: &str,
        env_vars: &[&str],
    ) -> Result<Self, ConfigError> {
        let overlays = env_vars
            .iter()
            .filter_map(|var| env::var(var).ok())
            .collect::<Vec<_>>();

        Self::from_toml_str_with_overlays(input, overlays.iter().map(String::as_str))
    }

    pub fn render_effective_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn validate(&self) -> Result<(), ConfigValidationErrors> {
        let mut errors = Vec::new();

        if self.app.name.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyAppName);
        }

        if self.server.bind.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyServerBind);
        }

        if self.i18n.supported_locales.is_empty() {
            errors.push(ConfigValidationError::MissingSupportedLocales);
        } else {
            if !self
                .i18n
                .supported_locales
                .contains(&self.i18n.default_locale)
            {
                errors.push(ConfigValidationError::DefaultLocaleNotSupported {
                    default_locale: self.i18n.default_locale.clone(),
                    supported_locales: self.i18n.supported_locales.clone(),
                });
            }

            if !self
                .i18n
                .supported_locales
                .contains(&self.i18n.fallback_locale)
            {
                errors.push(ConfigValidationError::FallbackLocaleNotSupported {
                    fallback_locale: self.i18n.fallback_locale.clone(),
                    supported_locales: self.i18n.supported_locales.clone(),
                });
            }
        }

        if self.seo.canonical_host.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyCanonicalHost);
        }

        if self.auth.package.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyAuthPackage);
        }

        if self.wasm.default_time_limit_ms == 0 {
            errors.push(ConfigValidationError::InvalidWasmTimeLimit);
        }

        if self.storage.local_root.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyLocalStorageRoot);
        }

        if self.assets.publish_manifest {
            match self.assets.cdn_base_url.as_deref() {
                Some(url) if url.starts_with("https://") || url.starts_with("http://") => {}
                Some(url) => errors.push(ConfigValidationError::InvalidCdnBaseUrl {
                    url: url.to_string(),
                }),
                None => errors.push(ConfigValidationError::MissingCdnBaseUrl),
            }
        }

        if self.modules.enabled.is_empty() {
            errors.push(ConfigValidationError::NoModulesEnabled);
        }

        match self.tls.mode {
            TlsMode::External => {
                if self.tls.challenge.is_some() {
                    errors.push(ConfigValidationError::TlsChallengeNotAllowed {
                        mode: self.tls.mode,
                    });
                }
            }
            TlsMode::Acme => {
                if self.tls.challenge.is_none() {
                    errors.push(ConfigValidationError::MissingTlsChallenge);
                }

                if self.tls.provider == Some(TlsProvider::CloudflareOriginCa) {
                    errors.push(ConfigValidationError::IncompatibleTlsProvider {
                        mode: self.tls.mode,
                        provider: TlsProvider::CloudflareOriginCa,
                    });
                }

                if self.tls.challenge == Some(AcmeChallenge::Dns01) && self.tls.provider.is_none() {
                    errors.push(ConfigValidationError::MissingDnsAutomationProvider);
                }
            }
            TlsMode::CloudflareOrigin => {
                if self.tls.provider != Some(TlsProvider::CloudflareOriginCa) {
                    errors.push(ConfigValidationError::CloudflareOriginRequiresOriginCa);
                }
            }
            TlsMode::Manual => {
                if self.tls.provider != Some(TlsProvider::ManualImport) {
                    errors.push(ConfigValidationError::ManualTlsRequiresManualProvider);
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigValidationErrors(errors))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub name: String,
    pub environment: Environment,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServerConfig {
    pub bind: String,
    #[serde(default)]
    pub trusted_proxies: Vec<String>,
    #[serde(default)]
    pub max_body_bytes: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TlsConfig {
    pub mode: TlsMode,
    #[serde(default)]
    pub challenge: Option<AcmeChallenge>,
    #[serde(default)]
    pub provider: Option<TlsProvider>,
    #[serde(default)]
    pub account_secret: Option<SecretRef>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsMode {
    External,
    Acme,
    CloudflareOrigin,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AcmeChallenge {
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
    CloudflareDns,
    CloudflareOriginCa,
    ManualImport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageConfig {
    pub default_class: StorageClass,
    #[serde(default)]
    pub object_store: Option<ObjectStoreKind>,
    pub local_root: String,
    #[serde(default)]
    pub object_store_secret: Option<SecretRef>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageClass {
    PublicAsset,
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectStoreKind {
    S3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheConfig {
    pub l1: CacheL1,
    #[serde(default)]
    pub l2: Option<DistributedCache>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheL1 {
    Moka,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCache {
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct I18nConfig {
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    pub fallback_locale: String,
    #[serde(default)]
    pub localized_routes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SeoConfig {
    pub canonical_host: String,
    pub emit_json_ld: bool,
    #[serde(default = "default_sitemap_enabled")]
    pub sitemap_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthConfig {
    pub package: String,
    pub explain_api: bool,
    #[serde(default)]
    pub tuple_store_secret: Option<SecretRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModulesConfig {
    pub enabled: Vec<String>,
    #[serde(flatten, default)]
    pub settings: toml::Table,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WasmConfig {
    pub directory: String,
    pub default_time_limit_ms: u64,
    pub allow_network: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobsConfig {
    pub backend: JobBackend,
    #[serde(default = "default_retry_limit")]
    pub retry_limit: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobBackend {
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservabilityConfig {
    pub metrics: bool,
    pub tracing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsConfig {
    pub publish_manifest: bool,
    #[serde(default)]
    pub cdn_base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretRef {
    Env { var: String },
    SecretManager { provider: String, key: String },
}

impl SecretRef {
    pub fn redacted(&self) -> String {
        match self {
            Self::Env { var } => format!("env:{var}"),
            Self::SecretManager { provider, key } => format!("secret-manager:{provider}:{key}"),
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error(transparent)]
    Validation(#[from] ConfigValidationErrors),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidationErrors(pub Vec<ConfigValidationError>);

impl fmt::Display for ConfigValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined = self
            .0
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        f.write_str(&joined)
    }
}

impl std::error::Error for ConfigValidationErrors {}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ConfigValidationError {
    #[error("app.name must not be empty")]
    EmptyAppName,
    #[error("server.bind must not be empty")]
    EmptyServerBind,
    #[error("at least one supported locale must be configured")]
    MissingSupportedLocales,
    #[error("default locale `{default_locale}` is not in supported_locales {supported_locales:?}")]
    DefaultLocaleNotSupported {
        default_locale: String,
        supported_locales: Vec<String>,
    },
    #[error(
        "fallback locale `{fallback_locale}` is not in supported_locales {supported_locales:?}"
    )]
    FallbackLocaleNotSupported {
        fallback_locale: String,
        supported_locales: Vec<String>,
    },
    #[error("seo.canonical_host must not be empty")]
    EmptyCanonicalHost,
    #[error("auth.package must not be empty")]
    EmptyAuthPackage,
    #[error("wasm.default_time_limit_ms must be greater than zero")]
    InvalidWasmTimeLimit,
    #[error("storage.local_root must not be empty")]
    EmptyLocalStorageRoot,
    #[error("assets.publish_manifest requires assets.cdn_base_url")]
    MissingCdnBaseUrl,
    #[error("assets.cdn_base_url must start with http:// or https://, got `{url}`")]
    InvalidCdnBaseUrl { url: String },
    #[error("at least one module must be enabled")]
    NoModulesEnabled,
    #[error("tls.challenge is required when tls.mode=acme")]
    MissingTlsChallenge,
    #[error("tls.challenge is not valid when tls.mode={mode:?}")]
    TlsChallengeNotAllowed { mode: TlsMode },
    #[error("dns-01 ACME requires a DNS automation provider")]
    MissingDnsAutomationProvider,
    #[error("tls.mode={mode:?} cannot be used with provider {provider:?}")]
    IncompatibleTlsProvider {
        mode: TlsMode,
        provider: TlsProvider,
    },
    #[error("tls.mode=cloudflare-origin requires provider=cloudflare-origin-ca")]
    CloudflareOriginRequiresOriginCa,
    #[error("tls.mode=manual requires provider=manual-import")]
    ManualTlsRequiresManualProvider,
}

fn default_sitemap_enabled() -> bool {
    true
}

fn default_retry_limit() -> u32 {
    5
}

use std::fmt;

fn merge_toml_value(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            for (key, value) in overlay_table {
                match base_table.get_mut(&key) {
                    Some(existing) => merge_toml_value(existing, value),
                    None => {
                        base_table.insert(key, value);
                    }
                }
            }
        }
        (base_value, overlay_value) => {
            *base_value = overlay_value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_CONFIG: &str = r#"
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
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false

[modules]
enabled = ["cms-pages", "admin-shell", "memberships", "events", "media-library"]

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
"#;

    #[test]
    fn parses_reference_config() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();

        assert_eq!(config.app.name, "showcase-events");
        assert_eq!(config.tls.mode, TlsMode::Acme);
        assert_eq!(config.tls.challenge, Some(AcmeChallenge::Dns01));
        assert_eq!(config.cache.l1, CacheL1::Moka);
        assert_eq!(config.cache.l2, Some(DistributedCache::Redis));
    }

    #[test]
    fn rejects_default_locale_outside_supported_list() {
        let invalid =
            VALID_CONFIG.replace("default_locale = \"en-GB\"", "default_locale = \"de-DE\"");

        let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

        match error {
            ConfigError::Validation(errors) => {
                assert!(errors.0.iter().any(|err| matches!(
                    err,
                    ConfigValidationError::DefaultLocaleNotSupported { .. }
                )));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_dns_01_without_provider() {
        let invalid = VALID_CONFIG.replace("provider = \"cloudflare-dns\"\n", "");

        let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

        match error {
            ConfigError::Validation(errors) => {
                assert!(
                    errors
                        .0
                        .contains(&ConfigValidationError::MissingDnsAutomationProvider)
                );
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn rejects_manifest_publishing_without_cdn_base_url() {
        let invalid = VALID_CONFIG.replace("cdn_base_url = \"https://cdn.example.com\"\n", "");

        let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

        match error {
            ConfigError::Validation(errors) => {
                assert!(errors.0.contains(&ConfigValidationError::MissingCdnBaseUrl));
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn overlay_toml_can_override_nested_values() {
        let overlay = r#"
[cache]
l2 = "valkey"

[seo]
canonical_host = "preview.example.com"
"#;

        let config = PlatformConfig::from_toml_str_with_overlays(VALID_CONFIG, [overlay]).unwrap();

        assert_eq!(config.cache.l2, Some(DistributedCache::Valkey));
        assert_eq!(config.seo.canonical_host, "preview.example.com");
    }

    #[test]
    fn rendered_effective_config_contains_applied_values() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let rendered = config.render_effective_toml().unwrap();

        assert!(rendered.contains("showcase-events"));
        assert!(rendered.contains("platform-default-auth"));
        assert!(rendered.contains("cdn.example.com"));
    }
}
