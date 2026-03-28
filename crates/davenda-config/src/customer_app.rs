use crate::PlatformConfig;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomerAppBootstrapManifest {
    app: CustomerAppBootstrapApp,
    domains: CustomerAppBootstrapDomains,
    i18n: CustomerAppBootstrapI18n,
    #[serde(default)]
    sites: Vec<CustomerAppBootstrapSite>,
    #[serde(default)]
    translations: CustomerAppBootstrapTranslations,
    auth: CustomerAppBootstrapAuth,
    modules: CustomerAppBootstrapModules,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapApp {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapDomains {
    #[serde(default)]
    canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapI18n {
    #[serde(default)]
    default_locale: String,
    #[serde(default)]
    supported_locales: Vec<String>,
    #[serde(default)]
    localized_routes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
struct CustomerAppBootstrapTranslations {
    #[serde(default)]
    catalogs: Vec<CustomerAppBootstrapTranslationCatalog>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomerAppBootstrapTranslationCatalog {
    locale: String,
    path: String,
}

impl CustomerAppBootstrapTranslationCatalog {
    pub fn new(locale: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            locale: locale.into(),
            path: path.into(),
        }
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomerAppBootstrapSite {
    id: String,
    display_name: String,
    #[serde(default)]
    brand_name: Option<String>,
    #[serde(alias = "canonical_domain")]
    canonical_host: String,
    #[serde(default, alias = "additional_domains")]
    hosts: Vec<String>,
    default_locale: String,
    supported_locales: Vec<String>,
    #[serde(default)]
    localized_routes: Option<bool>,
}

impl CustomerAppBootstrapSite {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        brand_name: Option<String>,
        canonical_host: impl Into<String>,
        hosts: Vec<String>,
        default_locale: impl Into<String>,
        supported_locales: Vec<String>,
        localized_routes: Option<bool>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            brand_name,
            canonical_host: canonical_host.into(),
            hosts,
            default_locale: default_locale.into(),
            supported_locales,
            localized_routes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapAuth {
    package: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapModules {
    enabled: Vec<String>,
}

#[derive(Debug, Error)]
pub enum CustomerAppBootstrapManifestError {
    #[error("customer app manifest `{path}` could not be read: {reason}")]
    Read { path: PathBuf, reason: String },
    #[error("customer app manifest `{path}` could not be parsed: {reason}")]
    Parse { path: PathBuf, reason: String },
    #[error(
        "customer app manifest app `{manifest}` does not match runtime config app `{configured}`"
    )]
    AppMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest auth package `{manifest}` does not match runtime config auth package `{configured}`"
    )]
    AuthPackageMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest default locale `{manifest}` does not match runtime config default locale `{configured}`"
    )]
    DefaultLocaleMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest supported locales `{manifest:?}` do not match runtime config supported locales `{configured:?}`"
    )]
    SupportedLocalesMismatch {
        manifest: Vec<String>,
        configured: Vec<String>,
    },
    #[error(
        "customer app manifest canonical host `{manifest}` does not match runtime config canonical host `{configured}`"
    )]
    CanonicalHostMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest modules differ from runtime config modules: manifest_only={manifest_only:?}, configured_only={configured_only:?}"
    )]
    ModulesMismatch {
        manifest_only: Vec<String>,
        configured_only: Vec<String>,
    },
    #[error(
        "customer app manifest sites differ from runtime config sites: manifest_only={manifest_only:?}, configured_only={configured_only:?}"
    )]
    SitesMismatch {
        manifest_only: Vec<String>,
        configured_only: Vec<String>,
    },
    #[error(
        "customer app manifest site `{site}` differs from runtime config site `{site}` for field `{field}`: manifest=`{manifest}`, configured=`{configured}`"
    )]
    SiteFieldMismatch {
        site: String,
        field: &'static str,
        manifest: String,
        configured: String,
    },
}

impl CustomerAppBootstrapManifest {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, CustomerAppBootstrapManifestError> {
        let path = path.as_ref();
        let source = std::fs::read_to_string(path).map_err(|error| {
            CustomerAppBootstrapManifestError::Read {
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        toml::from_str(&source).map_err(|error| CustomerAppBootstrapManifestError::Parse {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
    }

    pub fn new(
        app_name: impl Into<String>,
        default_locale: impl Into<String>,
        supported_locales: Vec<String>,
        localized_routes: bool,
        translation_catalogs: Vec<CustomerAppBootstrapTranslationCatalog>,
        auth_package: impl Into<String>,
        enabled_modules: Vec<String>,
        sites: Vec<CustomerAppBootstrapSite>,
        canonical_host: impl Into<String>,
    ) -> Self {
        Self {
            app: CustomerAppBootstrapApp {
                name: app_name.into(),
            },
            domains: CustomerAppBootstrapDomains {
                canonical: canonical_host.into(),
            },
            i18n: CustomerAppBootstrapI18n {
                default_locale: default_locale.into(),
                supported_locales,
                localized_routes,
            },
            translations: CustomerAppBootstrapTranslations {
                catalogs: translation_catalogs,
            },
            sites,
            auth: CustomerAppBootstrapAuth {
                package: auth_package.into(),
            },
            modules: CustomerAppBootstrapModules {
                enabled: enabled_modules,
            },
        }
    }

    pub fn enabled_modules(&self) -> &[String] {
        &self.modules.enabled
    }

    pub fn translation_catalogs(&self) -> &[CustomerAppBootstrapTranslationCatalog] {
        &self.translations.catalogs
    }

    pub fn supported_locales(&self) -> Vec<String> {
        let mut locales = if self.sites.is_empty() {
            self.i18n.supported_locales.clone()
        } else {
            self.resolved_sites()
                .into_iter()
                .flat_map(|site| site.supported_locales)
                .collect::<Vec<_>>()
        };
        locales.sort();
        locales.dedup();
        locales
    }

    fn resolved_sites(&self) -> Vec<CustomerAppBootstrapSite> {
        if !self.sites.is_empty() {
            return self.sites.clone();
        }

        vec![CustomerAppBootstrapSite::new(
            "default",
            self.app.name.clone(),
            None,
            self.domains.canonical.clone(),
            Vec::new(),
            self.i18n.default_locale.clone(),
            self.i18n.supported_locales.clone(),
            Some(self.i18n.localized_routes),
        )]
    }

    pub fn validate_runtime_config_alignment(
        &self,
        config: &PlatformConfig,
    ) -> Result<(), CustomerAppBootstrapManifestError> {
        if self.app.name != config.app.name {
            return Err(CustomerAppBootstrapManifestError::AppMismatch {
                manifest: self.app.name.clone(),
                configured: config.app.name.clone(),
            });
        }
        if self.auth.package != config.auth.package {
            return Err(CustomerAppBootstrapManifestError::AuthPackageMismatch {
                manifest: self.auth.package.clone(),
                configured: config.auth.package.clone(),
            });
        }
        if self.sites.is_empty() && config.sites.is_empty() {
            if self.i18n.default_locale != config.i18n.default_locale {
                return Err(CustomerAppBootstrapManifestError::DefaultLocaleMismatch {
                    manifest: self.i18n.default_locale.clone(),
                    configured: config.i18n.default_locale.clone(),
                });
            }

            let manifest_locales = sorted_strings(self.i18n.supported_locales.clone());
            let configured_locales = sorted_strings(config.i18n.supported_locales.clone());
            if manifest_locales != configured_locales {
                return Err(
                    CustomerAppBootstrapManifestError::SupportedLocalesMismatch {
                        manifest: manifest_locales,
                        configured: configured_locales,
                    },
                );
            }

            if self.domains.canonical != config.seo.canonical_host {
                return Err(CustomerAppBootstrapManifestError::CanonicalHostMismatch {
                    manifest: self.domains.canonical.clone(),
                    configured: config.seo.canonical_host.clone(),
                });
            }
        }

        let manifest_resolved_sites = self.resolved_sites();
        let configured_resolved_sites = if config.sites.is_empty() {
            let manifest_default = manifest_resolved_sites.first().cloned();
            vec![crate::SiteConfig {
                id: "default".to_string(),
                display_name: manifest_default
                    .as_ref()
                    .map(|site| site.display_name.clone())
                    .unwrap_or_else(|| config.app.name.clone()),
                brand_name: manifest_default
                    .as_ref()
                    .and_then(|site| site.brand_name.clone()),
                canonical_host: config.seo.canonical_host.clone(),
                hosts: Vec::new(),
                default_locale: config.i18n.default_locale.clone(),
                supported_locales: config.i18n.supported_locales.clone(),
                localized_routes: Some(config.i18n.localized_routes),
            }]
        } else {
            config.sites.clone()
        };

        let manifest_sites = sorted_strings(
            manifest_resolved_sites
                .iter()
                .map(|site| site.id.clone())
                .collect::<Vec<_>>(),
        );
        let configured_sites = sorted_strings(
            configured_resolved_sites
                .iter()
                .map(|site| site.id.clone())
                .collect::<Vec<_>>(),
        );
        let manifest_only = difference(&manifest_sites, &configured_sites);
        let configured_only = difference(&configured_sites, &manifest_sites);
        if !manifest_only.is_empty() || !configured_only.is_empty() {
            return Err(CustomerAppBootstrapManifestError::SitesMismatch {
                manifest_only,
                configured_only,
            });
        }

        for site in &manifest_resolved_sites {
            let configured = configured_resolved_sites
                .iter()
                .find(|configured| configured.id == site.id)
                .expect("site sets already aligned");
            if site.display_name != configured.display_name {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "display_name",
                    manifest: site.display_name.clone(),
                    configured: configured.display_name.clone(),
                });
            }
            if site.brand_name != configured.brand_name {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "brand_name",
                    manifest: site.brand_name.clone().unwrap_or_default(),
                    configured: configured.brand_name.clone().unwrap_or_default(),
                });
            }
            if site.canonical_host != configured.canonical_host {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "canonical_host",
                    manifest: site.canonical_host.clone(),
                    configured: configured.canonical_host.clone(),
                });
            }
            let manifest_hosts = sorted_strings(site.hosts.clone());
            let configured_hosts = sorted_strings(configured.hosts.clone());
            if manifest_hosts != configured_hosts {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "hosts",
                    manifest: manifest_hosts.join(","),
                    configured: configured_hosts.join(","),
                });
            }
            if site.default_locale != configured.default_locale {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "default_locale",
                    manifest: site.default_locale.clone(),
                    configured: configured.default_locale.clone(),
                });
            }
            let manifest_locales = sorted_strings(site.supported_locales.clone());
            let configured_locales = sorted_strings(configured.supported_locales.clone());
            if manifest_locales != configured_locales {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "supported_locales",
                    manifest: manifest_locales.join(","),
                    configured: configured_locales.join(","),
                });
            }
            let manifest_localized_routes =
                site.localized_routes.unwrap_or(self.i18n.localized_routes);
            let configured_localized_routes = configured
                .localized_routes
                .unwrap_or(config.i18n.localized_routes);
            if manifest_localized_routes != configured_localized_routes {
                return Err(CustomerAppBootstrapManifestError::SiteFieldMismatch {
                    site: site.id.clone(),
                    field: "localized_routes",
                    manifest: manifest_localized_routes.to_string(),
                    configured: configured_localized_routes.to_string(),
                });
            }
        }

        let manifest_modules = sorted_strings(self.modules.enabled.clone());
        let configured_modules = sorted_strings(config.modules.enabled.clone());
        let manifest_only = difference(&manifest_modules, &configured_modules);
        let configured_only = difference(&configured_modules, &manifest_modules);
        if !manifest_only.is_empty() || !configured_only.is_empty() {
            return Err(CustomerAppBootstrapManifestError::ModulesMismatch {
                manifest_only,
                configured_only,
            });
        }

        Ok(())
    }
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values
}

fn difference(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|value| !right.contains(value))
        .cloned()
        .collect()
}
