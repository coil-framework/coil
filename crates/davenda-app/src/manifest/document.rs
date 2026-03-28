use super::*;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomerAppManifestDocument {
    pub app: AppDocument,
    #[serde(default)]
    pub domains: DomainsDocument,
    #[serde(default)]
    pub i18n: I18nDocument,
    #[serde(default)]
    pub sites: Vec<SiteDocument>,
    pub theme: ThemeDocument,
    pub auth: AuthDocument,
    #[serde(default)]
    pub modules: ModulesDocument,
    #[serde(default)]
    pub content_models: Vec<ContentModelDocument>,
    #[serde(default)]
    pub customer_migrations: Vec<CustomerMigrationDocument>,
}

impl CustomerAppManifestDocument {
    pub fn from_toml_str(input: &str) -> Result<Self, AppModelError> {
        toml::from_str(input).map_err(|error| AppModelError::ManifestParse {
            message: error.to_string(),
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, AppModelError> {
        let path = path.as_ref();
        let input = fs::read_to_string(path).map_err(|error| AppModelError::ManifestRead {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Self::from_toml_str(&input)
    }

    pub fn into_manifest(self) -> Result<CustomerAppManifest, AppModelError> {
        let sites = self
            .sites
            .into_iter()
            .map(SiteDocument::into_site)
            .collect::<Result<Vec<_>, _>>()?;
        let app_id = CustomerAppId::new(self.app.name)?;
        let display_name = self.app.display_name.unwrap_or_else(|| app_id.to_string());
        let default_locale = if self.i18n.default_locale.is_empty() {
            sites.first()
                .map(|site| site.default_locale.clone())
                .ok_or_else(|| AppModelError::ManifestParse {
                    message: "customer app manifest requires [i18n].default_locale when no explicit [[sites]] are declared".to_string(),
                })?
        } else {
            LocaleTag::new(self.i18n.default_locale).map_err(|error| {
                AppModelError::ManifestParse {
                    message: error.to_string(),
                }
            })?
        };
        let supported_locales = if self.i18n.supported_locales.is_empty() {
            let mut locales = sites
                .iter()
                .flat_map(|site| site.supported_locales.clone())
                .collect::<Vec<_>>();
            locales.sort();
            locales.dedup();
            if locales.is_empty() {
                return Err(AppModelError::ManifestParse {
                    message: "customer app manifest requires [i18n].supported_locales when no explicit [[sites]] are declared".to_string(),
                });
            }
            locales
        } else {
            self.i18n
                .supported_locales
                .into_iter()
                .map(|locale| {
                    LocaleTag::new(locale).map_err(|error| AppModelError::ManifestParse {
                        message: error.to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let theme = build_theme_profile(self.theme)?;
        let auth = build_auth_strategy(self.auth)?;

        let mut manifest = CustomerAppManifest::new(
            app_id,
            display_name,
            default_locale,
            supported_locales,
            theme,
            auth,
        )?
        .with_localized_routes(self.i18n.localized_routes);

        if !self.domains.canonical.is_empty() {
            manifest = manifest.with_domain(AppDomain::new(self.domains.canonical, true)?);
        } else if sites.is_empty() {
            return Err(AppModelError::ManifestParse {
                message: "customer app manifest requires [domains].canonical when no explicit [[sites]] are declared".to_string(),
            });
        }
        for hostname in self.domains.additional {
            manifest = manifest.with_domain(AppDomain::new(hostname, false)?);
        }

        for site in sites {
            manifest = manifest.with_site(site);
        }

        for module in self.modules.enabled {
            manifest = manifest.with_module(InstalledModuleSpec::new(module)?);
        }

        for model in self.content_models {
            manifest = manifest.with_content_model(model.into_model()?);
        }

        for migration in self.customer_migrations {
            manifest = manifest.with_customer_migration(migration.into_contract()?);
        }

        manifest.validate()?;
        Ok(manifest)
    }
}

impl CustomerAppManifest {
    pub fn from_toml_str(input: &str) -> Result<Self, AppModelError> {
        CustomerAppManifestDocument::from_toml_str(input)?.into_manifest()
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, AppModelError> {
        CustomerAppManifestDocument::from_file(path)?.into_manifest()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AppDocument {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DomainsDocument {
    #[serde(default)]
    pub canonical: String,
    #[serde(default)]
    pub additional: Vec<String>,
}

impl Default for DomainsDocument {
    fn default() -> Self {
        Self {
            canonical: String::new(),
            additional: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct I18nDocument {
    #[serde(default)]
    pub default_locale: String,
    #[serde(default)]
    pub supported_locales: Vec<String>,
    #[serde(default)]
    pub localized_routes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SiteDocument {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub brand_name: Option<String>,
    pub canonical_domain: String,
    #[serde(default)]
    pub additional_domains: Vec<String>,
    pub default_locale: String,
    pub supported_locales: Vec<String>,
    #[serde(default)]
    pub localized_routes: Option<bool>,
}

impl SiteDocument {
    fn into_site(self) -> Result<AppSite, AppModelError> {
        let default_locale =
            LocaleTag::new(self.default_locale).map_err(|error| AppModelError::ManifestParse {
                message: error.to_string(),
            })?;
        let supported_locales = self
            .supported_locales
            .into_iter()
            .map(|locale| {
                LocaleTag::new(locale).map_err(|error| AppModelError::ManifestParse {
                    message: error.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut site = AppSite::new(
            self.id,
            self.display_name,
            default_locale,
            supported_locales,
        )?;
        if let Some(brand_name) = self.brand_name {
            site = site.with_brand_name(brand_name)?;
        }
        if let Some(localized_routes) = self.localized_routes {
            site = site.with_localized_routes(localized_routes);
        }
        site = site.with_domain(AppDomain::new(self.canonical_domain, true)?);
        for hostname in self.additional_domains {
            site = site.with_domain(AppDomain::new(hostname, false)?);
        }
        Ok(site)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ThemeDocument {
    pub active: String,
    pub template_namespaces: Vec<String>,
    #[serde(default)]
    pub asset_roots: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuthDocument {
    #[serde(default)]
    pub mode: Option<String>,
    pub package: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ModulesDocument {
    #[serde(default)]
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ContentModelDocument {
    pub id: String,
    pub resource_kind: String,
    pub fields: Vec<ContentFieldDocument>,
}

impl ContentModelDocument {
    fn into_model(self) -> Result<ContentModel, AppModelError> {
        let fields = self
            .fields
            .into_iter()
            .map(ContentFieldDocument::into_field)
            .collect::<Result<Vec<_>, _>>()?;
        ContentModel::new(self.id, self.resource_kind, fields)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ContentFieldDocument {
    pub id: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub localized: bool,
    #[serde(default)]
    pub required: bool,
}

impl ContentFieldDocument {
    fn into_field(self) -> Result<ContentField, AppModelError> {
        let field_type = parse_content_field_type(&self.field_type)?;
        let mut field = ContentField::new(self.id, field_type)?;
        if self.localized {
            field = field.localized();
        }
        if self.required {
            field = field.required();
        }
        Ok(field)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CustomerMigrationDocument {
    pub id: String,
    pub order: u16,
    pub description: String,
}

impl CustomerMigrationDocument {
    fn into_contract(self) -> Result<MigrationContract, AppModelError> {
        Ok(MigrationContract::new(
            self.id,
            self.order.into(),
            self.description,
        ))
    }
}

fn build_theme_profile(doc: ThemeDocument) -> Result<ThemeProfile, AppModelError> {
    let namespaces = doc
        .template_namespaces
        .into_iter()
        .map(|namespace| {
            TemplateNamespace::new(namespace).map_err(|error| AppModelError::ManifestParse {
                message: error.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut theme = ThemeProfile::new(ThemeId::new(doc.active)?, namespaces)?;
    for root in doc.asset_roots {
        theme = theme.with_asset_root(root)?;
    }
    Ok(theme)
}

fn build_auth_strategy(doc: AuthDocument) -> Result<AuthStrategy, AppModelError> {
    let mode = match doc
        .mode
        .as_deref()
        .unwrap_or("extend")
        .to_ascii_lowercase()
        .as_str()
    {
        "extend" => AuthMode::Extend,
        "replace" => AuthMode::Replace,
        other => {
            return Err(AppModelError::InvalidToken {
                field: "auth_mode",
                value: other.to_string(),
            });
        }
    };
    AuthStrategy::new(mode, doc.package)
}

fn parse_content_field_type(value: &str) -> Result<ContentFieldType, AppModelError> {
    match value.to_ascii_lowercase().as_str() {
        "text" => Ok(ContentFieldType::Text),
        "rich_text" | "rich-text" => Ok(ContentFieldType::RichText),
        "slug" => Ok(ContentFieldType::Slug),
        "boolean" => Ok(ContentFieldType::Boolean),
        "integer" => Ok(ContentFieldType::Integer),
        "date_time" | "date-time" | "datetime" => Ok(ContentFieldType::DateTime),
        "asset" => Ok(ContentFieldType::Asset),
        "reference" => Ok(ContentFieldType::Reference),
        other => Err(AppModelError::InvalidToken {
            field: "content_field_type",
            value: other.to_string(),
        }),
    }
}
