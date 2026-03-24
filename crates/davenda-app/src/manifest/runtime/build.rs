use super::*;
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
        let app_root = std::env::current_dir().map_err(|error| AppModelError::RuntimeBuild {
            message: format!("failed to resolve customer app root: {error}"),
        })?;
        self.build_runtime_plan_with_extensions_at(
            config,
            auth_package,
            modules,
            extension_packages,
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

        let mut builder = RuntimeBuilder::new(config.clone(), auth_package);
        builder = builder.with_template_root(app_root);
        for module in modules {
            builder = builder.with_boxed_module(module);
        }
        for extension in installed_extensions {
            builder = builder.with_installed_extension(extension);
        }

        let runtime = builder.build()?;
        let theme_publication = self.publish_theme_assets(&config, &runtime, app_root)?;

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
        if config.app.name != self.id.as_str() {
            return Err(AppModelError::ConfigAppMismatch {
                manifest: self.id.to_string(),
                configured: config.app.name.clone(),
            });
        }

        if config.auth.package != self.auth.package_name {
            return Err(AppModelError::ConfigAuthPackageMismatch {
                manifest: self.auth.package_name.clone(),
                configured: config.auth.package.clone(),
            });
        }

        if config.i18n.default_locale != self.default_locale.as_str() {
            return Err(AppModelError::ConfigDefaultLocaleMismatch {
                manifest: self.default_locale.to_string(),
                configured: config.i18n.default_locale.clone(),
            });
        }

        let manifest_locales = sorted_locale_strings(&self.supported_locales);
        let configured_locales = sorted_strings(config.i18n.supported_locales.clone());
        if manifest_locales != configured_locales {
            return Err(AppModelError::ConfigSupportedLocalesMismatch {
                manifest: manifest_locales,
                configured: configured_locales,
            });
        }

        let canonical_domain = self
            .domains
            .iter()
            .find(|domain| domain.canonical)
            .expect("validated manifests always declare a canonical domain")
            .hostname
            .clone();
        if config.seo.canonical_host != canonical_domain {
            return Err(AppModelError::ConfigCanonicalHostMismatch {
                manifest: canonical_domain,
                configured: config.seo.canonical_host.clone(),
            });
        }

        let manifest_modules = sorted_strings(
            self.modules
                .iter()
                .map(|module| module.id.to_string())
                .collect::<Vec<_>>(),
        );
        let configured_modules = sorted_strings(config.modules.enabled.clone());
        let manifest_only = difference(&manifest_modules, &configured_modules);
        let configured_only = difference(&configured_modules, &manifest_modules);
        if !manifest_only.is_empty() || !configured_only.is_empty() {
            return Err(AppModelError::ConfigModulesMismatch {
                manifest_only,
                configured_only,
            });
        }

        Ok(())
    }

    fn publish_theme_assets<A>(
        &self,
        config: &PlatformConfig,
        runtime: &RuntimePlan,
        app_root: A,
    ) -> Result<Option<davenda_assets::ThemeAssetPublicationReceipt>, AppModelError>
    where
        A: AsRef<Path>,
    {
        if !config.assets.publish_manifest || self.theme.asset_roots().is_empty() {
            return Ok(None);
        }

        let release_id = davenda_assets::ReleaseId::new(format!(
            "{}-{}-theme-assets",
            self.id, self.theme.active
        ))?;
        let publication = self.theme.publication_plan(release_id, app_root)?;
        let resolver = davenda_runtime::EnvironmentSecretResolver;
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

fn validate_customer_app_root(app_root: &Path) -> Result<(), AppModelError> {
    if !app_root.exists() {
        return Err(AppModelError::RuntimeBuild {
            message: format!(
                "customer app root `{}` does not exist",
                app_root.display()
            ),
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
