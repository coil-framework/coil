use super::*;
use coil_config::{CustomerAppBootstrapManifest, CustomerAppBootstrapManifestError};
use coil_customer_sdk::CustomerBackendPlugin;
use coil_i18n::TranslationCatalog;
use std::path::Path;

impl CustomerAppManifest {
    pub fn build_runtime_plan<P>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
    {
        self.build_runtime_plan_with_extensions(config, auth_package, modules, Vec::new())
    }

    pub fn build_runtime_plan_at<P, A>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        app_root: A,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
        A: AsRef<Path>,
    {
        self.build_runtime_plan_with_extensions_at(
            config,
            auth_package,
            modules,
            Vec::new(),
            app_root,
        )
    }

    pub fn build_runtime_plan_with_extensions<P>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        extension_packages: Vec<ExtensionPackage>,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
    {
        self.build_runtime_plan_with_extensions_and_customer_plugins(
            config,
            auth_package,
            modules,
            extension_packages,
            Vec::new(),
        )
    }

    pub fn build_runtime_plan_with_customer_plugins<P, A>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
        app_root: A,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
        A: AsRef<Path>,
    {
        self.build_runtime_plan_with_extensions_and_customer_plugins_at(
            config,
            auth_package,
            modules,
            Vec::new(),
            customer_plugins,
            app_root,
        )
    }

    pub fn build_runtime_plan_with_extensions_and_customer_plugins<P>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        extension_packages: Vec<ExtensionPackage>,
        customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
    {
        let app_root = std::env::current_dir().map_err(|error| AppModelError::RuntimeBuild {
            message: format!("failed to resolve customer app root: {error}"),
        })?;
        self.build_runtime_plan_with_extensions_and_customer_plugins_at(
            config,
            auth_package,
            modules,
            extension_packages,
            customer_plugins,
            app_root,
        )
    }

    pub fn build_runtime_plan_with_extensions_at<P, A>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        extension_packages: Vec<ExtensionPackage>,
        app_root: A,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
        A: AsRef<Path>,
    {
        self.build_runtime_plan_with_extensions_and_customer_plugins_at(
            config,
            auth_package,
            modules,
            extension_packages,
            Vec::new(),
            app_root,
        )
    }

    pub fn build_runtime_plan_with_extensions_and_customer_plugins_at<P, A>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        extension_packages: Vec<ExtensionPackage>,
        customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
        app_root: A,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
        A: AsRef<Path>,
    {
        let app_root = app_root.as_ref();
        validate_customer_app_root(app_root)?;
        self.validate_runtime_config_alignment(&config)?;

        let manifests = modules
            .iter()
            .map(|module| module.manifest())
            .collect::<Vec<_>>();
        let unexpected_modules = sorted_strings(
            manifests
                .iter()
                .filter(|manifest| {
                    !self
                        .modules
                        .iter()
                        .any(|installed| installed.id.as_str() == manifest.name)
                })
                .map(|manifest| manifest.name.clone())
                .collect::<Vec<_>>(),
        );
        if !unexpected_modules.is_empty() {
            return Err(AppModelError::UnexpectedRuntimeModules {
                app_id: self.id.to_string(),
                modules: unexpected_modules,
            });
        }

        let composition = self.compose(&auth_package, &manifests)?;
        let migration_summary =
            build_migration_summary(self, auth_package.manifest().name.clone(), &modules);
        let release_doctor = self.release_doctor_with_extensions(
            &auth_package,
            &manifests,
            &extension_packages,
            Some(&config),
        )?;
        let installed_extensions = self.resolve_extension_packages(&extension_packages)?;
        let translation_catalogs = load_customer_translation_catalogs(self, app_root)?;

        let mut builder = RuntimeBuilder::new(config.clone(), auth_package);
        builder = builder.with_template_root(app_root);
        builder = builder.with_translation_catalogs(translation_catalogs);
        for module in modules {
            builder = builder.with_boxed_module(module);
        }
        for plugin in customer_plugins {
            builder = builder.with_boxed_customer_plugin(plugin);
        }
        for extension in installed_extensions {
            builder = builder.with_installed_extension(extension);
        }

        let runtime = builder.build()?;
        let theme_publication = self.publish_theme_assets(&config, &runtime, app_root)?;
        let mut runtime = runtime;
        runtime.theme_asset_manifest = theme_publication
            .as_ref()
            .map(|publication| publication.manifest().clone());

        Ok(CustomerAppRuntimePlan {
            composition,
            runtime,
            theme_publication,
            migration_summary,
            release_doctor,
        })
    }

    pub fn build_customer_root_runtime_plan_with_extensions_and_customer_plugins_at<P, A>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        extension_packages: Vec<ExtensionPackage>,
        customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
        app_root: A,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage + 'static,
        A: AsRef<Path>,
    {
        let app_root = app_root.as_ref();
        validate_customer_app_root(app_root)?;
        self.validate_runtime_config_alignment(&config)?;

        let manifests = modules
            .iter()
            .map(|module| module.manifest())
            .collect::<Vec<_>>();
        let unexpected_modules = sorted_strings(
            manifests
                .iter()
                .filter(|manifest| {
                    !self
                        .modules
                        .iter()
                        .any(|installed| installed.id.as_str() == manifest.name)
                })
                .map(|manifest| manifest.name.clone())
                .collect::<Vec<_>>(),
        );
        if !unexpected_modules.is_empty() {
            return Err(AppModelError::UnexpectedRuntimeModules {
                app_id: self.id.to_string(),
                modules: unexpected_modules,
            });
        }

        let composition = self.compose(&auth_package, &manifests)?;
        let migration_summary =
            build_migration_summary(self, auth_package.manifest().name.clone(), &modules);
        let release_doctor = self.release_doctor_with_extensions(
            &auth_package,
            &manifests,
            &extension_packages,
            Some(&config),
        )?;
        let installed_extensions = self.resolve_extension_packages(&extension_packages)?;
        let translation_catalogs = load_customer_translation_catalogs(self, app_root)?;

        let mut builder = RuntimeBuilder::for_customer_root(config.clone(), auth_package)
            .with_customer_root(app_root)
            .with_translation_catalogs(translation_catalogs);
        for module in modules {
            builder = builder.with_boxed_module(module);
        }
        for plugin in customer_plugins {
            builder = builder.with_boxed_linked_customer_plugin(plugin);
        }
        for extension in installed_extensions {
            builder = builder.with_installed_extension(extension);
        }

        let runtime = builder.build()?;
        let theme_publication = self.publish_theme_assets(&config, &runtime, app_root)?;
        let mut runtime = runtime;
        runtime.theme_asset_manifest = theme_publication
            .as_ref()
            .map(|publication| publication.manifest().clone());

        Ok(CustomerAppRuntimePlan {
            composition,
            runtime,
            theme_publication,
            migration_summary,
            release_doctor,
        })
    }

    pub fn migration_summary<P>(
        &self,
        auth_package: P,
        modules: &[Box<dyn PlatformModule>],
    ) -> MigrationPlanSummary
    where
        P: AuthModelPackage + 'static,
    {
        build_migration_summary(self, auth_package.manifest().name.clone(), modules)
    }

    pub fn validate_runtime_config_alignment(
        &self,
        config: &PlatformConfig,
    ) -> Result<(), AppModelError> {
        let resolved_sites = self.resolved_sites()?;
        let compatibility_canonical_host = self
            .domains
            .iter()
            .find(|domain| domain.canonical)
            .map(|domain| domain.hostname.clone())
            .or_else(|| {
                resolved_sites
                    .first()
                    .and_then(|site| site.canonical_domain().map(ToString::to_string))
            })
            .expect("validated manifests always resolve a canonical domain");
        let bootstrap_manifest = CustomerAppBootstrapManifest::new(
            self.id.to_string(),
            self.default_locale.to_string(),
            sorted_locale_strings(&self.supported_locales),
            self.localized_routes,
            self.translations
                .iter()
                .map(|catalog| {
                    coil_config::CustomerAppBootstrapTranslationCatalog::new(
                        catalog.locale.to_string(),
                        catalog.path.clone(),
                    )
                })
                .collect::<Vec<_>>(),
            self.auth.package_name.clone(),
            self.modules
                .iter()
                .map(|module| module.id.to_string())
                .collect::<Vec<_>>(),
            resolved_sites
                .iter()
                .map(|site| {
                    coil_config::CustomerAppBootstrapSite::new(
                        site.id.to_string(),
                        site.display_name.clone(),
                        site.brand_name.clone(),
                        site.canonical_domain()
                            .expect("validated app sites always have a canonical domain")
                            .to_string(),
                        site.domains
                            .iter()
                            .filter(|domain| !domain.canonical)
                            .map(|domain| domain.hostname.clone())
                            .collect::<Vec<_>>(),
                        site.default_locale.to_string(),
                        sorted_locale_strings(&site.supported_locales),
                        site.localized_routes,
                    )
                })
                .collect::<Vec<_>>(),
            compatibility_canonical_host,
        );
        bootstrap_manifest
            .validate_runtime_config_alignment(config)
            .map_err(customer_bootstrap_manifest_error_into_app_model)
    }

    fn publish_theme_assets<A>(
        &self,
        config: &PlatformConfig,
        runtime: &RuntimePlan,
        app_root: A,
    ) -> Result<Option<coil_assets::ThemeAssetPublicationReceipt>, AppModelError>
    where
        A: AsRef<Path>,
    {
        if !config.assets.publish_manifest || self.theme.asset_roots().is_empty() {
            return Ok(None);
        }

        let release_id = coil_assets::ReleaseId::new(format!(
            "{}-{}-theme-assets",
            self.id, self.theme.active
        ))?;
        let publication = self.theme.publication_plan(release_id, app_root)?;
        let resolver = coil_runtime::EnvironmentSecretResolver;
        let object_store = runtime
            .object_store_client_config(&resolver)
            .map_err(|error| AppModelError::RuntimeBuild {
                message: format!(
                    "failed to resolve build-time storage backends for `{}`: {error}",
                    self.id
                ),
            })?;
        let receipt = runtime
            .storage_host_with_object_store(object_store)
            .publish_theme_assets(&publication)
            .map_err(|error| AppModelError::RuntimeBuild {
                message: format!("failed to publish theme assets for `{}`: {error}", self.id),
            })?;

        Ok(Some(receipt))
    }
}

fn load_customer_translation_catalogs(
    manifest: &CustomerAppManifest,
    app_root: &Path,
) -> Result<Vec<TranslationCatalog>, AppModelError> {
    manifest
        .translations
        .iter()
        .map(|catalog| {
            let path = app_root.join(&catalog.path);
            TranslationCatalog::from_toml_file(catalog.locale.clone(), &path).map_err(|error| {
                AppModelError::RuntimeBuild {
                    message: format!(
                        "failed to load customer translation catalog `{}` for locale `{}`: {error}",
                        catalog.path,
                        catalog.locale
                    ),
                }
            })
        })
        .collect()
}

fn customer_bootstrap_manifest_error_into_app_model(
    error: CustomerAppBootstrapManifestError,
) -> AppModelError {
    match error {
        CustomerAppBootstrapManifestError::AppMismatch {
            manifest,
            configured,
        } => AppModelError::ConfigAppMismatch {
            manifest,
            configured,
        },
        CustomerAppBootstrapManifestError::AuthPackageMismatch {
            manifest,
            configured,
        } => AppModelError::ConfigAuthPackageMismatch {
            manifest,
            configured,
        },
        CustomerAppBootstrapManifestError::DefaultLocaleMismatch {
            manifest,
            configured,
        } => AppModelError::ConfigDefaultLocaleMismatch {
            manifest,
            configured,
        },
        CustomerAppBootstrapManifestError::SupportedLocalesMismatch {
            manifest,
            configured,
        } => AppModelError::ConfigSupportedLocalesMismatch {
            manifest,
            configured,
        },
        CustomerAppBootstrapManifestError::CanonicalHostMismatch {
            manifest,
            configured,
        } => AppModelError::ConfigCanonicalHostMismatch {
            manifest,
            configured,
        },
        CustomerAppBootstrapManifestError::ModulesMismatch {
            manifest_only,
            configured_only,
        } => AppModelError::ConfigModulesMismatch {
            manifest_only,
            configured_only,
        },
        CustomerAppBootstrapManifestError::SitesMismatch {
            manifest_only,
            configured_only,
        } => AppModelError::ConfigSitesMismatch {
            manifest_only,
            configured_only,
        },
        CustomerAppBootstrapManifestError::SiteFieldMismatch {
            site,
            field,
            manifest,
            configured,
        } => AppModelError::ConfigSiteFieldMismatch {
            site,
            field,
            manifest,
            configured,
        },
        CustomerAppBootstrapManifestError::Read { path, reason }
        | CustomerAppBootstrapManifestError::Parse { path, reason } => {
            AppModelError::RuntimeBuild {
                message: format!(
                    "failed to read shared customer bootstrap manifest model from `{}`: {reason}",
                    path.display()
                ),
            }
        }
    }
}

fn validate_customer_app_root(app_root: &Path) -> Result<(), AppModelError> {
    if !app_root.exists() {
        return Err(AppModelError::RuntimeBuild {
            message: format!("customer app root `{}` does not exist", app_root.display()),
        });
    }
    if !app_root.is_dir() {
        return Err(AppModelError::RuntimeBuild {
            message: format!(
                "customer app root `{}` is not a directory",
                app_root.display()
            ),
        });
    }

    let templates_root = app_root.join("templates");
    if !templates_root.exists() {
        return Err(AppModelError::RuntimeBuild {
            message: format!(
                "customer app templates directory `{}` does not exist",
                templates_root.display()
            ),
        });
    }
    if !templates_root.is_dir() {
        return Err(AppModelError::RuntimeBuild {
            message: format!(
                "customer app templates directory `{}` is not a directory",
                templates_root.display()
            ),
        });
    }

    Ok(())
}
