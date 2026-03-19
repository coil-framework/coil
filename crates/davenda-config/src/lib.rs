use std::env;
use std::fs;
use std::net::SocketAddr;
use std::path::Path;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;
mod validation;

pub use validation::{ConfigValidationError, ConfigValidationErrors};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlatformConfig {
    pub app: AppConfig,
    pub server: ServerConfig,
    pub http: HttpConfig,
    pub tls: TlsConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
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

impl ServerConfig {
    pub fn trusts_forwarded_headers(&self, remote_addr: Option<&SocketAddr>) -> bool {
        let Some(remote_addr) = remote_addr else {
            return false;
        };

        self.trusted_proxies.iter().any(|trusted_proxy| {
            trusted_proxy
                .parse::<IpNet>()
                .map(|network| network.contains(&remote_addr.ip()))
                .unwrap_or(false)
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HttpConfig {
    pub session: SessionConfig,
    pub session_cookie: CookieConfig,
    pub flash_cookie: CookieConfig,
    pub csrf: CsrfConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    pub store: SessionStore,
    pub idle_timeout_secs: u64,
    pub absolute_timeout_secs: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStore {
    Memory,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CookieConfig {
    pub name: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default = "default_cookie_path")]
    pub path: String,
    pub same_site: SameSitePolicy,
    #[serde(default = "default_true")]
    pub secure: bool,
    #[serde(default = "default_true")]
    pub http_only: bool,
    #[serde(default)]
    pub protection: CookieProtection,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SameSitePolicy {
    Lax,
    Strict,
    None,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CookieProtection {
    #[default]
    Signed,
    Encrypted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CsrfConfig {
    pub enabled: bool,
    pub field_name: String,
    pub header_name: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_driver")]
    pub driver: DatabaseDriver,
    #[serde(default = "default_database_url_secret")]
    pub url: Option<SecretRef>,
    #[serde(default = "default_database_schema")]
    pub schema: String,
    #[serde(default = "default_migrations_table")]
    pub migrations_table: String,
    #[serde(default = "default_min_database_connections")]
    pub min_connections: u16,
    #[serde(default = "default_max_database_connections")]
    pub max_connections: u16,
    #[serde(default = "default_statement_timeout_secs")]
    pub statement_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            driver: default_database_driver(),
            url: default_database_url_secret(),
            schema: default_database_schema(),
            migrations_table: default_migrations_table(),
            min_connections: default_min_database_connections(),
            max_connections: default_max_database_connections(),
            statement_timeout_secs: default_statement_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseDriver {
    Postgres,
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
    pub tenant_id: i64,
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

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error(transparent)]
    Validation(#[from] ConfigValidationErrors),
}

fn default_sitemap_enabled() -> bool {
    true
}

fn default_retry_limit() -> u32 {
    5
}

fn default_database_driver() -> DatabaseDriver {
    DatabaseDriver::Postgres
}

fn default_database_url_secret() -> Option<SecretRef> {
    Some(SecretRef::Env {
        var: "DATABASE_URL".to_string(),
    })
}

fn default_database_schema() -> String {
    "public".to_string()
}

fn default_migrations_table() -> String {
    "_davenda_migrations".to_string()
}

fn default_min_database_connections() -> u16 {
    4
}

fn default_max_database_connections() -> u16 {
    32
}

fn default_statement_timeout_secs() -> u64 {
    30
}

fn default_cookie_path() -> String {
    "/".to_string()
}

fn default_true() -> bool {
    true
}

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
