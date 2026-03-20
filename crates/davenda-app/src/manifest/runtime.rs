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
        if config.app.name != self.id.as_str() {
            return Err(AppModelError::ConfigAppMismatch {
                manifest: self.id.to_string(),
                configured: config.app.name,
            });
        }

        if config.auth.package != self.auth.package_name {
            return Err(AppModelError::ConfigAuthPackageMismatch {
                manifest: self.auth.package_name.clone(),
                configured: config.auth.package,
            });
        }

        if config.i18n.default_locale != self.default_locale.as_str() {
            return Err(AppModelError::ConfigDefaultLocaleMismatch {
                manifest: self.default_locale.to_string(),
                configured: config.i18n.default_locale,
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
                configured: config.seo.canonical_host,
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

    pub fn release_doctor_with_extensions<P>(
        &self,
        auth_package: &P,
        manifests: &[ModuleManifest],
        packages: &[ExtensionPackage],
        config: Option<&PlatformConfig>,
    ) -> Result<ReleaseDoctorReport, AppModelError>
    where
        P: AuthModelPackage + 'static,
    {
        let composition = self.compose(auth_package, manifests)?;
        let mut report = composition.release_doctor(config);
        self.append_extension_doctor_findings(packages, &mut report);
        Ok(report)
    }

    fn resolve_extension_packages(
        &self,
        packages: &[ExtensionPackage],
    ) -> Result<Vec<davenda_wasm::InstalledExtension>, AppModelError> {
        if self.extensions.is_empty() {
            return Ok(Vec::new());
        }

        if packages.is_empty() {
            return Err(AppModelError::ExtensionPackagesRequired {
                app_id: self.id.to_string(),
            });
        }

        let mut installed = Vec::new();
        for extension in &self.extensions {
            let package = packages
                .iter()
                .find(|package| package.id().as_str() == extension.id.as_str())
                .ok_or_else(|| AppModelError::UnknownExtensionPackage {
                    app_id: self.id.to_string(),
                    extension_id: extension.id.to_string(),
                })?;

            if package.version() != extension.package_version {
                return Err(AppModelError::ExtensionVersionMismatch {
                    extension_id: extension.id.to_string(),
                    configured: extension.package_version,
                    actual: package.version(),
                });
            }

            if package.artifact_sha256 != extension.artifact_sha256 {
                return Err(AppModelError::ExtensionArtifactChecksumMismatch {
                    extension_id: extension.id.to_string(),
                    configured: extension.artifact_sha256.clone(),
                    actual: package.artifact_sha256.clone(),
                });
            }

            installed.push(package.install(extension.installation.clone(), &extension.config)?);
        }

        Ok(installed)
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

    fn append_extension_doctor_findings(
        &self,
        packages: &[ExtensionPackage],
        report: &mut ReleaseDoctorReport,
    ) {
        if self.extensions.is_empty() {
            return;
        }

        if packages.is_empty() {
            report.findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "extension.packages.missing",
                format!(
                    "customer app `{}` installs extensions but no extension packages were supplied",
                    self.id
                ),
            ));
            return;
        }

        for extension in &self.extensions {
            let Some(package) = packages
                .iter()
                .find(|package| package.id().as_str() == extension.id.as_str())
            else {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.package.unknown",
                    format!(
                        "customer extension `{}` does not have a matching package artifact",
                        extension.id
                    ),
                ));
                continue;
            };

            if package.version() != extension.package_version {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.version.mismatch",
                    format!(
                        "customer extension `{}` pins version `{}` but package provides `{}`",
                        extension.id,
                        extension.package_version,
                        package.version()
                    ),
                ));
            }

            if package.artifact_sha256 != extension.artifact_sha256 {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.checksum.mismatch",
                    format!(
                        "customer extension `{}` pins digest `{}` but package provides `{}`",
                        extension.id, extension.artifact_sha256, package.artifact_sha256
                    ),
                ));
            }

            if let Err(error) = package.config_schema.effective_values(&extension.config) {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.config.invalid",
                    format!(
                        "customer extension `{}` has invalid config: {error}",
                        extension.id
                    ),
                ));
            }
        }
    }
}
