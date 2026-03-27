use crate::PlatformConfig;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomerAppBootstrapManifest {
    app: CustomerAppBootstrapApp,
    domains: CustomerAppBootstrapDomains,
    i18n: CustomerAppBootstrapI18n,
    auth: CustomerAppBootstrapAuth,
    modules: CustomerAppBootstrapModules,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapApp {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapDomains {
    canonical: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct CustomerAppBootstrapI18n {
    default_locale: String,
    supported_locales: Vec<String>,
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
        canonical_host: impl Into<String>,
        default_locale: impl Into<String>,
        supported_locales: Vec<String>,
        auth_package: impl Into<String>,
        enabled_modules: Vec<String>,
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
            },
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
