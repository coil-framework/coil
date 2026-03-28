use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppManifest {
    pub id: CustomerAppId,
    pub display_name: String,
    pub domains: Vec<AppDomain>,
    pub sites: Vec<AppSite>,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub localized_routes: bool,
    pub modules: Vec<InstalledModuleSpec>,
    pub theme: ThemeProfile,
    pub auth: AuthStrategy,
    pub translations: Vec<AppTranslationCatalog>,
    pub content_models: Vec<ContentModel>,
    pub customer_migrations: Vec<MigrationContract>,
    pub extensions: Vec<CustomerExtension>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTranslationCatalog {
    pub locale: LocaleTag,
    pub path: String,
}

impl AppTranslationCatalog {
    pub fn new(locale: LocaleTag, path: impl Into<String>) -> Result<Self, AppModelError> {
        let path = require_non_empty("translation_catalog_path", path.into())?;
        if std::path::Path::new(&path).is_absolute() {
            return Err(AppModelError::InvalidRelativePath {
                field: "translation_catalog_path",
                value: path,
            });
        }
        Ok(Self { locale, path })
    }
}

impl CustomerAppManifest {
    pub fn new(
        id: CustomerAppId,
        display_name: impl Into<String>,
        default_locale: LocaleTag,
        supported_locales: Vec<LocaleTag>,
        theme: ThemeProfile,
        auth: AuthStrategy,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id,
            display_name: require_non_empty("display_name", display_name.into())?,
            domains: Vec::new(),
            sites: Vec::new(),
            default_locale,
            supported_locales,
            localized_routes: false,
            modules: Vec::new(),
            theme,
            auth,
            translations: Vec::new(),
            content_models: Vec::new(),
            customer_migrations: Vec::new(),
            extensions: Vec::new(),
        })
    }

    pub fn with_domain(mut self, domain: AppDomain) -> Self {
        self.domains.push(domain);
        self
    }

    pub fn with_site(mut self, site: AppSite) -> Self {
        self.sites.push(site);
        self
    }

    pub fn with_localized_routes(mut self, localized_routes: bool) -> Self {
        self.localized_routes = localized_routes;
        self
    }

    pub fn with_module(mut self, module: InstalledModuleSpec) -> Self {
        self.modules.push(module);
        self
    }

    pub fn with_content_model(mut self, model: ContentModel) -> Self {
        self.content_models.push(model);
        self
    }

    pub fn with_translation_catalog(mut self, catalog: AppTranslationCatalog) -> Self {
        self.translations.push(catalog);
        self
    }

    pub fn with_customer_migration(mut self, migration: MigrationContract) -> Self {
        self.customer_migrations.push(migration);
        self
    }

    pub fn with_extension(mut self, extension: CustomerExtension) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn validate(&self) -> Result<(), AppModelError> {
        if !self
            .supported_locales
            .iter()
            .any(|locale| locale == &self.default_locale)
        {
            return Err(AppModelError::DefaultLocaleNotSupported {
                default_locale: self.default_locale.to_string(),
            });
        }

        let mut domains = BTreeSet::new();
        let mut canonical_domains = 0usize;
        for domain in &self.domains {
            if !domains.insert(domain.hostname.clone()) {
                return Err(AppModelError::DuplicateDomain {
                    domain: domain.hostname.clone(),
                });
            }
            if domain.canonical {
                canonical_domains += 1;
            }
        }
        if canonical_domains == 0 && self.sites.is_empty() {
            return Err(AppModelError::MissingCanonicalDomain {
                app_id: self.id.to_string(),
            });
        }

        let mut sites = BTreeSet::new();
        let mut all_site_domains = BTreeSet::new();
        for site in &self.sites {
            if !sites.insert(site.id.to_string()) {
                return Err(AppModelError::DuplicateSite {
                    site: site.id.to_string(),
                });
            }
            if !site
                .supported_locales
                .iter()
                .any(|locale| locale == &site.default_locale)
            {
                return Err(AppModelError::SiteDefaultLocaleNotSupported {
                    site: site.id.to_string(),
                    default_locale: site.default_locale.to_string(),
                });
            }

            for locale in &site.supported_locales {
                if !self
                    .supported_locales
                    .iter()
                    .any(|supported| supported == locale)
                {
                    return Err(AppModelError::SiteLocaleOutsideAppSupport {
                        site: site.id.to_string(),
                        locale: locale.to_string(),
                    });
                }
            }

            let mut site_domains = BTreeSet::new();
            let mut canonical_site_domains = 0usize;
            for domain in &site.domains {
                if !site_domains.insert(domain.hostname.clone()) {
                    return Err(AppModelError::DuplicateDomain {
                        domain: domain.hostname.clone(),
                    });
                }
                if !all_site_domains.insert(domain.hostname.clone()) {
                    return Err(AppModelError::DuplicateSiteDomain {
                        domain: domain.hostname.clone(),
                    });
                }
                if domain.canonical {
                    canonical_site_domains += 1;
                }
            }
            if canonical_site_domains == 0 {
                return Err(AppModelError::MissingCanonicalSiteDomain {
                    app_id: self.id.to_string(),
                    site: site.id.to_string(),
                });
            }
        }

        let mut modules = BTreeSet::new();
        for module in &self.modules {
            if !modules.insert(module.id.to_string()) {
                return Err(AppModelError::DuplicateInstalledModule {
                    module: module.id.to_string(),
                });
            }
        }

        let mut content_models = BTreeSet::new();
        for model in &self.content_models {
            if !content_models.insert(model.id.to_string()) {
                return Err(AppModelError::DuplicateContentModel {
                    model_id: model.id.to_string(),
                });
            }
        }

        let mut extensions = BTreeSet::new();
        for extension in &self.extensions {
            if !extensions.insert(extension.id.to_string()) {
                return Err(AppModelError::DuplicateExtension {
                    extension_id: extension.id.to_string(),
                });
            }

            if extension.installation.customer_app_id != self.id.as_str() {
                return Err(AppModelError::ExtensionCustomerAppMismatch {
                    extension_id: extension.id.to_string(),
                    extension_customer_app: extension.installation.customer_app_id.clone(),
                    app_id: self.id.to_string(),
                });
            }
        }

        let mut translation_catalog_locales = BTreeSet::new();
        for catalog in &self.translations {
            if !self
                .supported_locales
                .iter()
                .any(|supported| supported == &catalog.locale)
            {
                return Err(AppModelError::TranslationCatalogLocaleOutsideAppSupport {
                    locale: catalog.locale.to_string(),
                });
            }
            if !translation_catalog_locales.insert(catalog.locale.to_string()) {
                return Err(AppModelError::DuplicateTranslationCatalog {
                    locale: catalog.locale.to_string(),
                });
            }
        }

        Ok(())
    }

    pub fn resolved_sites(&self) -> Result<Vec<AppSite>, AppModelError> {
        if !self.sites.is_empty() {
            return Ok(self.sites.clone());
        }

        let mut site = AppSite::new(
            "default",
            self.display_name.clone(),
            self.default_locale.clone(),
            self.supported_locales.clone(),
        )?
        .with_localized_routes(self.localized_routes);
        for domain in &self.domains {
            site = site.with_domain(domain.clone());
        }
        Ok(vec![site])
    }
}
