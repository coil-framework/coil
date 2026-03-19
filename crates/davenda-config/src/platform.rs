use std::env;
use std::fs;
use std::path::Path;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
