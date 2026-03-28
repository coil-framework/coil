use std::fmt;

use ipnet::IpNet;
use thiserror::Error;

use super::*;

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
    #[error("server.trusted_proxies contains invalid entry `{value}`")]
    InvalidTrustedProxy { value: String },
    #[error("{field} must be greater than zero")]
    InvalidSessionTimeout { field: &'static str },
    #[error(
        "http.session.absolute_timeout_secs ({absolute_timeout_secs}) must be at least idle_timeout_secs ({idle_timeout_secs})"
    )]
    AbsoluteSessionTimeoutTooShort {
        idle_timeout_secs: u64,
        absolute_timeout_secs: u64,
    },
    #[error("{cookie}.name must not be empty")]
    EmptyCookieName { cookie: &'static str },
    #[error("{cookie}.path must start with `/`, got `{path}`")]
    InvalidCookiePath { cookie: &'static str, path: String },
    #[error("{cookie} must be secure when same_site=none")]
    SameSiteNoneRequiresSecure { cookie: &'static str },
    #[error("http.csrf.field_name must not be empty when CSRF is enabled")]
    EmptyCsrfFieldName,
    #[error("http.csrf.header_name must not be empty when CSRF is enabled")]
    EmptyCsrfHeaderName,
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
    #[error("site `{site}` display_name must not be empty")]
    EmptySiteDisplayName { site: String },
    #[error("site `{site}` canonical_host must not be empty")]
    EmptySiteCanonicalHost { site: String },
    #[error("site `{site}` hosts must not be empty")]
    MissingSiteHosts { site: String },
    #[error("site `{site}` is declared more than once")]
    DuplicateSite { site: String },
    #[error("site host `{host}` is declared more than once")]
    DuplicateSiteHost { host: String },
    #[error(
        "site `{site}` default locale `{default_locale}` is not in supported_locales {supported_locales:?}"
    )]
    SiteDefaultLocaleNotSupported {
        site: String,
        default_locale: String,
        supported_locales: Vec<String>,
    },
    #[error(
        "site `{site}` locale `{locale}` is not in app supported_locales {supported_locales:?}"
    )]
    SiteLocaleOutsideAppSupport {
        site: String,
        locale: String,
        supported_locales: Vec<String>,
    },
    #[error("auth.package must not be empty")]
    EmptyAuthPackage,
    #[error("auth.tenant_id must be greater than zero, got {tenant_id}")]
    InvalidAuthTenantId { tenant_id: i64 },
    #[error("wasm.default_time_limit_ms must be greater than zero")]
    InvalidWasmTimeLimit,
    #[error("wasm.outbound_http contains an entry with an empty integration name")]
    EmptyWasmOutboundHttpIntegration,
    #[error("wasm.outbound_http contains duplicate integration `{integration}`")]
    DuplicateWasmOutboundHttpIntegration { integration: String },
    #[error(
        "wasm.outbound_http integration `{integration}` must use http or https, got `{scheme}`"
    )]
    InvalidWasmOutboundHttpScheme { integration: String, scheme: String },
    #[error("wasm.outbound_http integration `{integration}` must include a host")]
    MissingWasmOutboundHttpHost { integration: String },
    #[error("wasm.outbound_http integration `{integration}` must not include embedded credentials")]
    WasmOutboundHttpHasCredentials { integration: String },
    #[error("storage.local_root must not be empty")]
    EmptyLocalStorageRoot,
    #[error(
        "storage.default_class={storage_class:?} requires storage.single_node_escape_hatch=explicit_single_node because local-only storage must be explicitly enabled"
    )]
    LocalOnlyStorageRequiresExplicitOptIn { storage_class: StorageClass },
    #[error(
        "storage.single_node_escape_hatch=explicit_single_node requires storage.deployment=single_node because local-only storage is not permitted in distributed deployments"
    )]
    LocalOnlyStorageRequiresSingleNodeDeployment,
    #[error("database.schema must not be empty")]
    EmptyDatabaseSchema,
    #[error("database.migrations_table must not be empty")]
    EmptyMigrationsTable,
    #[error(
        "database pool sizing is invalid: min_connections={min_connections} max_connections={max_connections}"
    )]
    InvalidDatabasePoolSize {
        min_connections: u16,
        max_connections: u16,
    },
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
    #[error(
        "http.session.store={store:?} requires cache.l2={store:?} semantics, got {cache_backend:?}"
    )]
    SessionStoreRequiresDistributedCache {
        store: SessionStore,
        cache_backend: Option<DistributedCache>,
    },
}

impl PlatformConfig {
    pub fn validate(&self) -> Result<(), ConfigValidationErrors> {
        let mut errors = Vec::new();

        if self.app.name.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyAppName);
        }

        if self.server.bind.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyServerBind);
        }

        for trusted_proxy in &self.server.trusted_proxies {
            if trusted_proxy.parse::<IpNet>().is_err() {
                errors.push(ConfigValidationError::InvalidTrustedProxy {
                    value: trusted_proxy.clone(),
                });
            }
        }

        if self.http.session.idle_timeout_secs == 0 {
            errors.push(ConfigValidationError::InvalidSessionTimeout {
                field: "http.session.idle_timeout_secs",
            });
        }

        if self.http.session.absolute_timeout_secs == 0 {
            errors.push(ConfigValidationError::InvalidSessionTimeout {
                field: "http.session.absolute_timeout_secs",
            });
        }

        if self.http.session.absolute_timeout_secs < self.http.session.idle_timeout_secs {
            errors.push(ConfigValidationError::AbsoluteSessionTimeoutTooShort {
                idle_timeout_secs: self.http.session.idle_timeout_secs,
                absolute_timeout_secs: self.http.session.absolute_timeout_secs,
            });
        }

        for (cookie_name, cookie) in [
            ("http.session_cookie", &self.http.session_cookie),
            ("http.flash_cookie", &self.http.flash_cookie),
        ] {
            if cookie.name.trim().is_empty() {
                errors.push(ConfigValidationError::EmptyCookieName {
                    cookie: cookie_name,
                });
            }

            if cookie.path.trim().is_empty() || !cookie.path.starts_with('/') {
                errors.push(ConfigValidationError::InvalidCookiePath {
                    cookie: cookie_name,
                    path: cookie.path.clone(),
                });
            }

            if cookie.same_site == SameSitePolicy::None && !cookie.secure {
                errors.push(ConfigValidationError::SameSiteNoneRequiresSecure {
                    cookie: cookie_name,
                });
            }
        }

        if self.http.csrf.enabled {
            if self.http.csrf.field_name.trim().is_empty() {
                errors.push(ConfigValidationError::EmptyCsrfFieldName);
            }

            if self.http.csrf.header_name.trim().is_empty() {
                errors.push(ConfigValidationError::EmptyCsrfHeaderName);
            }
        }

        let has_explicit_sites = !self.sites.is_empty();
        if self.i18n.supported_locales.is_empty() {
            if !has_explicit_sites {
                errors.push(ConfigValidationError::MissingSupportedLocales);
            }
        } else {
            if self.i18n.default_locale.trim().is_empty()
                || !self
                    .i18n
                    .supported_locales
                    .contains(&self.i18n.default_locale)
            {
                errors.push(ConfigValidationError::DefaultLocaleNotSupported {
                    default_locale: self.i18n.default_locale.clone(),
                    supported_locales: self.i18n.supported_locales.clone(),
                });
            }

            if self.i18n.fallback_locale.trim().is_empty()
                || !self
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

        if self.seo.canonical_host.trim().is_empty() && !has_explicit_sites {
            errors.push(ConfigValidationError::EmptyCanonicalHost);
        }

        let mut site_ids = std::collections::BTreeSet::new();
        let mut site_hosts = std::collections::BTreeSet::new();
        for site in &self.sites {
            if site.id.trim().is_empty() {
                errors.push(ConfigValidationError::DuplicateSite {
                    site: site.id.clone(),
                });
            } else if !site_ids.insert(site.id.clone()) {
                errors.push(ConfigValidationError::DuplicateSite {
                    site: site.id.clone(),
                });
            }

            if site.display_name.trim().is_empty() {
                errors.push(ConfigValidationError::EmptySiteDisplayName {
                    site: site.id.clone(),
                });
            }

            if site.canonical_host.trim().is_empty() {
                errors.push(ConfigValidationError::EmptySiteCanonicalHost {
                    site: site.id.clone(),
                });
            }

            if site.hosts.is_empty() {
                errors.push(ConfigValidationError::MissingSiteHosts {
                    site: site.id.clone(),
                });
            }

            if !site.supported_locales.contains(&site.default_locale) {
                errors.push(ConfigValidationError::SiteDefaultLocaleNotSupported {
                    site: site.id.clone(),
                    default_locale: site.default_locale.clone(),
                    supported_locales: site.supported_locales.clone(),
                });
            }

            for locale in &site.supported_locales {
                if !self.i18n.supported_locales.is_empty()
                    && !self.i18n.supported_locales.contains(locale)
                {
                    errors.push(ConfigValidationError::SiteLocaleOutsideAppSupport {
                        site: site.id.clone(),
                        locale: locale.clone(),
                        supported_locales: self.i18n.supported_locales.clone(),
                    });
                }
            }

            for host in site
                .hosts
                .iter()
                .cloned()
                .chain(std::iter::once(site.canonical_host.clone()))
            {
                if !site_hosts.insert(host.clone()) {
                    errors.push(ConfigValidationError::DuplicateSiteHost { host });
                }
            }
        }

        if self.auth.package.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyAuthPackage);
        }

        if self.auth.tenant_id <= 0 {
            errors.push(ConfigValidationError::InvalidAuthTenantId {
                tenant_id: self.auth.tenant_id,
            });
        }

        if self.wasm.default_time_limit_ms == 0 {
            errors.push(ConfigValidationError::InvalidWasmTimeLimit);
        }

        let mut outbound_http_integrations = std::collections::BTreeSet::new();
        for integration in &self.wasm.outbound_http {
            if integration.integration.trim().is_empty() {
                errors.push(ConfigValidationError::EmptyWasmOutboundHttpIntegration);
                continue;
            }

            if !outbound_http_integrations.insert(integration.integration.clone()) {
                errors.push(
                    ConfigValidationError::DuplicateWasmOutboundHttpIntegration {
                        integration: integration.integration.clone(),
                    },
                );
            }

            match integration.endpoint.scheme() {
                "http" | "https" => {}
                scheme => {
                    errors.push(ConfigValidationError::InvalidWasmOutboundHttpScheme {
                        integration: integration.integration.clone(),
                        scheme: scheme.to_string(),
                    });
                }
            }

            if integration.endpoint.host_str().is_none() {
                errors.push(ConfigValidationError::MissingWasmOutboundHttpHost {
                    integration: integration.integration.clone(),
                });
            }

            if !integration.endpoint.username().is_empty()
                || integration.endpoint.password().is_some()
            {
                errors.push(ConfigValidationError::WasmOutboundHttpHasCredentials {
                    integration: integration.integration.clone(),
                });
            }
        }

        if self.storage.local_root.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyLocalStorageRoot);
        }

        if matches!(self.storage.default_class, StorageClass::LocalOnlySensitive)
            && self.storage.single_node_escape_hatch != SingleNodeStorageMode::ExplicitSingleNode
        {
            errors.push(
                ConfigValidationError::LocalOnlyStorageRequiresExplicitOptIn {
                    storage_class: self.storage.default_class,
                },
            );
        }

        if self.storage.single_node_escape_hatch == SingleNodeStorageMode::ExplicitSingleNode
            && self.storage.deployment != StorageDeployment::SingleNode
        {
            errors.push(ConfigValidationError::LocalOnlyStorageRequiresSingleNodeDeployment);
        }

        if self.database.schema.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyDatabaseSchema);
        }

        if self.database.migrations_table.trim().is_empty() {
            errors.push(ConfigValidationError::EmptyMigrationsTable);
        }

        if self.database.max_connections == 0
            || self.database.min_connections > self.database.max_connections
        {
            errors.push(ConfigValidationError::InvalidDatabasePoolSize {
                min_connections: self.database.min_connections,
                max_connections: self.database.max_connections,
            });
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

        match self.http.session.store {
            SessionStore::Redis => {
                if self.cache.l2 != Some(DistributedCache::Redis) {
                    errors.push(
                        ConfigValidationError::SessionStoreRequiresDistributedCache {
                            store: self.http.session.store,
                            cache_backend: self.cache.l2,
                        },
                    );
                }
            }
            SessionStore::Valkey => {
                if self.cache.l2 != Some(DistributedCache::Valkey) {
                    errors.push(
                        ConfigValidationError::SessionStoreRequiresDistributedCache {
                            store: self.http.session.store,
                            cache_backend: self.cache.l2,
                        },
                    );
                }
            }
            SessionStore::Memory | SessionStore::Database => {}
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
