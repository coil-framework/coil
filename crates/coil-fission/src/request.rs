#[cfg(feature = "server")]
use fission::server::RevalidationPolicy;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(feature = "server")]
use std::time::Duration;
use thiserror::Error;

/// One configured public site and its market/locale boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteDefinition {
    pub id: String,
    pub canonical_origin: String,
    pub hosts: Vec<String>,
    pub market: String,
    pub default_locale: String,
    pub supported_locales: BTreeSet<String>,
}

impl SiteDefinition {
    pub fn new(
        id: impl Into<String>,
        canonical_origin: impl Into<String>,
        market: impl Into<String>,
        default_locale: impl Into<String>,
    ) -> Self {
        let default_locale = default_locale.into();
        Self {
            id: id.into(),
            canonical_origin: canonical_origin.into(),
            hosts: Vec::new(),
            market: market.into(),
            supported_locales: BTreeSet::from([default_locale.clone()]),
            default_locale,
        }
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Self {
        self.hosts.push(host.into());
        self
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.supported_locales.insert(locale.into());
        self
    }
}

/// Validated request facts carried into Fission state and typed job requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoilRequestScope {
    pub site_id: String,
    pub market: String,
    pub locale: String,
    pub canonical_origin: String,
    pub request_host: String,
    pub route: String,
    pub session_id: String,
}

#[derive(Clone, Debug, Default)]
pub struct SiteRegistry {
    by_host: BTreeMap<String, SiteDefinition>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SiteRegistryError {
    #[error("site `{site}` has no configured hosts")]
    MissingHosts { site: String },
    #[error("site `{site}` has an invalid host `{host}`")]
    InvalidHost { site: String, host: String },
    #[error("host `{host}` is configured for both `{first}` and `{second}`")]
    DuplicateHost {
        host: String,
        first: String,
        second: String,
    },
    #[error("host `{host}` does not identify a configured Coil site")]
    UnknownHost { host: String },
    #[error("locale `{locale}` is not supported by site `{site}`")]
    UnsupportedLocale { site: String, locale: String },
}

impl SiteRegistry {
    pub fn new(sites: impl IntoIterator<Item = SiteDefinition>) -> Result<Self, SiteRegistryError> {
        let mut by_host = BTreeMap::<String, SiteDefinition>::new();
        for site in sites {
            if site.hosts.is_empty() {
                return Err(SiteRegistryError::MissingHosts {
                    site: site.id.clone(),
                });
            }
            for raw_host in &site.hosts {
                let host =
                    normalize_host(raw_host).ok_or_else(|| SiteRegistryError::InvalidHost {
                        site: site.id.clone(),
                        host: raw_host.clone(),
                    })?;
                if let Some(existing) = by_host.get(&host) {
                    return Err(SiteRegistryError::DuplicateHost {
                        host,
                        first: existing.id.clone(),
                        second: site.id.clone(),
                    });
                }
                by_host.insert(host, site.clone());
            }
        }
        Ok(Self { by_host })
    }

    pub fn resolve(
        &self,
        host: &str,
        requested_locale: Option<&str>,
        route: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Result<CoilRequestScope, SiteRegistryError> {
        let normalized = normalize_host(host).ok_or_else(|| SiteRegistryError::UnknownHost {
            host: host.to_string(),
        })?;
        let site = self
            .by_host
            .get(&normalized)
            .ok_or_else(|| SiteRegistryError::UnknownHost {
                host: normalized.clone(),
            })?;
        let locale = requested_locale.unwrap_or(&site.default_locale);
        if !site.supported_locales.contains(locale) {
            return Err(SiteRegistryError::UnsupportedLocale {
                site: site.id.clone(),
                locale: locale.to_string(),
            });
        }
        Ok(CoilRequestScope {
            site_id: site.id.clone(),
            market: site.market.clone(),
            locale: locale.to_string(),
            canonical_origin: site.canonical_origin.trim_end_matches('/').to_string(),
            request_host: normalized,
            route: route.into(),
            session_id: session_id.into(),
        })
    }
}

/// Safe public-page cache policy for multi-site Coil routes.
///
/// Fission already includes resolved locale and theme in its cache key. Coil
/// adds mandatory Host variance so two sites can never share cached HTML.
#[cfg(feature = "server")]
pub fn public_revalidation(
    ttl: Duration,
    tags: impl IntoIterator<Item = impl Into<fission::server::CacheTag>>,
) -> RevalidationPolicy {
    RevalidationPolicy::new(ttl).vary("host").tags(tags)
}

fn normalize_host(host: &str) -> Option<String> {
    let normalized = host.trim().trim_end_matches('.').to_ascii_lowercase();
    (!normalized.is_empty()
        && !normalized
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        && !normalized.contains(['/', '\\', '@']))
    .then_some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> SiteRegistry {
        SiteRegistry::new([
            SiteDefinition::new("shoppr-uk", "https://uk.shoppr.test", "GB", "en-GB")
                .with_host("uk.shoppr.test")
                .with_host("uk.localhost:8080"),
            SiteDefinition::new("shoppr-fr", "https://fr.shoppr.test", "FR", "fr-FR")
                .with_host("fr.shoppr.test")
                .with_locale("en-GB"),
        ])
        .unwrap()
    }

    #[test]
    fn resolves_site_market_locale_and_session_as_one_scope() {
        let scope = registry()
            .resolve(
                "UK.LOCALHOST:8080",
                None,
                "/products/linen-shirt",
                "session-1",
            )
            .unwrap();

        assert_eq!(scope.site_id, "shoppr-uk");
        assert_eq!(scope.market, "GB");
        assert_eq!(scope.locale, "en-GB");
        assert_eq!(scope.request_host, "uk.localhost:8080");
    }

    #[test]
    fn rejects_unknown_hosts_and_unsupported_locales() {
        assert!(matches!(
            registry().resolve("attacker.test", None, "/", "session-1"),
            Err(SiteRegistryError::UnknownHost { .. })
        ));
        assert!(matches!(
            registry().resolve("uk.shoppr.test", Some("pl-PL"), "/", "session-1"),
            Err(SiteRegistryError::UnsupportedLocale { .. })
        ));
    }

    #[test]
    fn refuses_ambiguous_host_ownership() {
        let error = SiteRegistry::new([
            SiteDefinition::new("one", "https://one.test", "GB", "en-GB").with_host("shared.test"),
            SiteDefinition::new("two", "https://two.test", "FR", "fr-FR").with_host("SHARED.TEST"),
        ])
        .unwrap_err();

        assert!(matches!(error, SiteRegistryError::DuplicateHost { .. }));
    }

    #[cfg(feature = "server")]
    #[test]
    fn public_cache_policy_always_varies_by_host() {
        let policy = public_revalidation(Duration::from_secs(60), ["catalog"]);
        assert_eq!(policy.vary, vec!["host"]);
        assert_eq!(policy.tags.len(), 1);
    }
}
