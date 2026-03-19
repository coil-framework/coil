use std::collections::{BTreeMap, HashMap};

use crate::validation::{require_non_empty, validate_route};
use crate::{I18nError, LocaleTag};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocaleRoutingStrategy {
    PathPrefix,
    Host,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleUrlConfig {
    pub strategy: LocaleRoutingStrategy,
    pub canonical_host: String,
    pub host_map: HashMap<LocaleTag, String>,
}

impl LocaleUrlConfig {
    pub fn path_prefix(canonical_host: impl Into<String>) -> Result<Self, I18nError> {
        Ok(Self {
            strategy: LocaleRoutingStrategy::PathPrefix,
            canonical_host: require_non_empty("canonical_host", canonical_host.into())?,
            host_map: HashMap::new(),
        })
    }

    pub fn host_map(
        canonical_host: impl Into<String>,
        host_map: HashMap<LocaleTag, String>,
    ) -> Result<Self, I18nError> {
        Ok(Self {
            strategy: LocaleRoutingStrategy::Host,
            canonical_host: require_non_empty("canonical_host", canonical_host.into())?,
            host_map,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedUrls {
    pub canonical: String,
    pub alternate_hreflang: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleRouter {
    config: LocaleUrlConfig,
}

impl LocaleRouter {
    pub fn new(config: LocaleUrlConfig) -> Self {
        Self { config }
    }

    pub fn localized_path(&self, locale: &LocaleTag, path: &str) -> Result<String, I18nError> {
        let normalized = validate_route("path", path.to_string())?;
        match self.config.strategy {
            LocaleRoutingStrategy::PathPrefix => Ok(format!(
                "/{}/{}",
                locale.as_str().trim_matches('/'),
                normalized.trim_start_matches('/')
            )),
            LocaleRoutingStrategy::Host => Ok(normalized),
            LocaleRoutingStrategy::Mixed => Ok(format!(
                "/{}/{}",
                locale.as_str().trim_matches('/'),
                normalized.trim_start_matches('/')
            )),
        }
    }

    pub fn absolute_url(&self, locale: &LocaleTag, path: &str) -> Result<String, I18nError> {
        let host = match self.config.strategy {
            LocaleRoutingStrategy::Host | LocaleRoutingStrategy::Mixed => self
                .config
                .host_map
                .get(locale)
                .cloned()
                .unwrap_or_else(|| self.config.canonical_host.clone()),
            LocaleRoutingStrategy::PathPrefix => self.config.canonical_host.clone(),
        };

        let localized_path = self.localized_path(locale, path)?;
        Ok(format!("https://{host}{localized_path}"))
    }

    pub fn alternate_urls(
        &self,
        locales: &[LocaleTag],
        path: &str,
    ) -> Result<LocalizedUrls, I18nError> {
        let canonical_locale = locales.first().ok_or_else(|| I18nError::MissingLocale {
            locale: "canonical".to_string(),
        })?;
        let canonical = self.absolute_url(canonical_locale, path)?;
        let mut alternate_hreflang = BTreeMap::new();

        for locale in locales {
            alternate_hreflang.insert(locale.to_string(), self.absolute_url(locale, path)?);
        }

        Ok(LocalizedUrls {
            canonical,
            alternate_hreflang,
        })
    }
}
