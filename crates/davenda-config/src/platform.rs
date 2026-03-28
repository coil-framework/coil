use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    AppConfig, AssetsConfig, CacheConfig, ConfigValidationErrors, DatabaseConfig, HttpConfig,
    JobsConfig, ObservabilityConfig, SecretRef, ServerConfig, StorageConfig, TlsConfig,
};

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
    #[serde(default)]
    pub i18n: I18nConfig,
    #[serde(default)]
    pub seo: SeoConfig,
    #[serde(default)]
    pub sites: Vec<SiteConfig>,
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

    pub fn site_for_host(&self, host: &str) -> Option<&SiteConfig> {
        self.sites.iter().find(|site| {
            site.canonical_host == host || site.hosts.iter().any(|value| value == host)
        })
    }

    pub fn site_for_id(&self, site_id: &str) -> Option<&SiteConfig> {
        self.sites.iter().find(|site| site.id == site_id)
    }

    pub fn default_site(&self) -> Option<&SiteConfig> {
        self.sites.first()
    }

    pub fn canonical_host_for_site(&self, site_id: Option<&str>) -> &str {
        site_id
            .and_then(|site_id| self.site_for_id(site_id))
            .or_else(|| self.default_site())
            .map(|site| site.canonical_host.as_str())
            .unwrap_or(self.seo.canonical_host.as_str())
    }

    pub fn default_locale_for_site(&self, site_id: Option<&str>) -> &str {
        site_id
            .and_then(|site_id| self.site_for_id(site_id))
            .or_else(|| self.default_site())
            .map(|site| site.default_locale.as_str())
            .unwrap_or(self.i18n.default_locale.as_str())
    }

    pub fn supported_locales_for_site(&self, site_id: Option<&str>) -> &[String] {
        site_id
            .and_then(|site_id| self.site_for_id(site_id))
            .or_else(|| self.default_site())
            .map(|site| site.supported_locales.as_slice())
            .unwrap_or(self.i18n.supported_locales.as_slice())
    }

    pub fn localized_routes_for_site(&self, site_id: Option<&str>) -> bool {
        site_id
            .and_then(|site_id| self.site_for_id(site_id))
            .or_else(|| self.default_site())
            .and_then(|site| site.localized_routes)
            .unwrap_or(self.i18n.localized_routes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct I18nConfig {
    #[serde(default)]
    pub default_locale: String,
    #[serde(default)]
    pub supported_locales: Vec<String>,
    #[serde(default)]
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

impl Default for SeoConfig {
    fn default() -> Self {
        Self {
            canonical_host: String::new(),
            emit_json_ld: false,
            sitemap_enabled: default_sitemap_enabled(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SiteConfig {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub brand_name: Option<String>,
    pub canonical_host: String,
    #[serde(default)]
    pub hosts: Vec<String>,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    #[serde(default)]
    pub localized_routes: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthConfig {
    pub package: String,
    pub explain_api: bool,
    pub tenant_id: i64,
    #[serde(default)]
    pub tuple_store_secret: Option<crate::SecretRef>,
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
    #[serde(default)]
    pub secret_bindings: BTreeMap<String, SecretRef>,
    #[serde(default)]
    pub outbound_http: Vec<WasmOutboundHttpIntegration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WasmOutboundHttpIntegration {
    pub integration: String,
    pub endpoint: Url,
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
