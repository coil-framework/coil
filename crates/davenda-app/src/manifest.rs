use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppManifest {
    pub id: CustomerAppId,
    pub display_name: String,
    pub domains: Vec<AppDomain>,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub modules: Vec<InstalledModuleSpec>,
    pub theme: ThemeProfile,
    pub auth: AuthStrategy,
    pub content_models: Vec<ContentModel>,
    pub customer_migrations: Vec<MigrationContract>,
    pub extensions: Vec<CustomerExtension>,
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
            default_locale,
            supported_locales,
            modules: Vec::new(),
            theme,
            auth,
            content_models: Vec::new(),
            customer_migrations: Vec::new(),
            extensions: Vec::new(),
        })
    }

    pub fn with_domain(mut self, domain: AppDomain) -> Self {
        self.domains.push(domain);
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
        if canonical_domains == 0 {
            return Err(AppModelError::MissingCanonicalDomain {
                app_id: self.id.to_string(),
            });
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

        Ok(())
    }

    pub fn compose<P>(
        &self,
        auth_package: &P,
        manifests: &[ModuleManifest],
    ) -> Result<CustomerAppComposition, AppModelError>
    where
        P: AuthModelPackage,
    {
        self.validate()?;

        if auth_package.manifest().name != self.auth.package_name {
            return Err(AppModelError::AuthPackageMismatch {
                app_id: self.id.to_string(),
                configured: self.auth.package_name.clone(),
                actual: auth_package.manifest().name.clone(),
            });
        }

        let installed_ids = self
            .modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>();

        let mut selected = Vec::new();
        for module in &self.modules {
            let manifest = manifests
                .iter()
                .find(|candidate| candidate.name == module.id.as_str())
                .ok_or_else(|| AppModelError::UnknownInstalledModule {
                    app_id: self.id.to_string(),
                    module: module.id.to_string(),
                })?;

            validate_module_capabilities(auth_package, manifest)?;
            selected.push(manifest.clone());
        }

        for manifest in &selected {
            for dependency in &manifest.module_dependencies {
                if dependency.kind == ModuleDependencyKind::Required
                    && !installed_ids
                        .iter()
                        .any(|module| module == &dependency.module)
                {
                    return Err(AppModelError::MissingModuleDependency {
                        module: manifest.name.clone(),
                        dependency: dependency.module.clone(),
                    });
                }
            }
        }

        let mut module_inventory = Vec::new();
        let mut required_core_services = Vec::new();
        let mut migrations = self.customer_migrations.clone();
        let mut route_surfaces = Vec::new();
        let mut jobs = Vec::new();
        let mut event_subscriptions = Vec::new();
        let mut admin_resources = Vec::new();
        let mut search_contributions = Vec::new();
        let mut report_definitions = Vec::new();
        let mut bulk_operations = Vec::new();

        for manifest in &selected {
            for dependency in &manifest.core_service_dependencies {
                if !required_core_services.contains(dependency) {
                    required_core_services.push(*dependency);
                }
            }

            migrations.extend(manifest.migrations.clone());
            route_surfaces.extend(manifest.route_surfaces.clone());
            jobs.extend(manifest.jobs.clone());
            event_subscriptions.extend(manifest.event_subscriptions.clone());
            admin_resources.extend(manifest.admin_resources.clone());
            search_contributions.extend(manifest.search_contributions.clone());
            report_definitions.extend(manifest.report_definitions.clone());
            bulk_operations.extend(manifest.bulk_operations.clone());

            let installed_spec = self
                .modules
                .iter()
                .find(|spec| spec.id.as_str() == manifest.name)
                .expect("selected manifests always correspond to installed modules");
            module_inventory.push(InstalledModuleSummary {
                id: installed_spec.id.clone(),
                version_req: installed_spec.version_req.clone(),
                module_dependencies: manifest.module_dependencies.clone(),
                core_service_dependencies: manifest.core_service_dependencies.clone(),
                migrations: manifest.migrations.clone(),
                route_surfaces: manifest.route_surfaces.clone(),
                jobs: manifest.jobs.clone(),
                event_subscriptions: manifest.event_subscriptions.clone(),
                admin_resources: manifest.admin_resources.clone(),
                search_contributions: manifest.search_contributions.clone(),
                report_definitions: manifest.report_definitions.clone(),
                bulk_operations: manifest.bulk_operations.clone(),
            });
        }

        Ok(CustomerAppComposition {
            app_id: self.id.clone(),
            display_name: self.display_name.clone(),
            domains: self.domains.clone(),
            default_locale: self.default_locale.clone(),
            supported_locales: self.supported_locales.clone(),
            installed_modules: self.modules.clone(),
            module_inventory,
            required_core_services,
            migrations,
            route_surfaces,
            jobs,
            event_subscriptions,
            admin_resources,
            search_contributions,
            report_definitions,
            bulk_operations,
            theme: self.theme.clone(),
            content_models: self.content_models.clone(),
            extensions: self.extensions.clone(),
            auth: self.auth.clone(),
        })
    }

    pub fn build_runtime_plan<P>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage,
    {
        self.build_runtime_plan_with_extensions(config, auth_package, modules, Vec::new())
    }

    pub fn build_runtime_plan_with_extensions<P>(
        &self,
        config: PlatformConfig,
        auth_package: P,
        modules: Vec<Box<dyn PlatformModule>>,
        extension_packages: Vec<ExtensionPackage>,
    ) -> Result<CustomerAppRuntimePlan, AppModelError>
    where
        P: AuthModelPackage,
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

        let mut builder = RuntimeBuilder::new(config, auth_package);
        for module in modules {
            builder = builder.with_boxed_module(module);
        }
        for extension in installed_extensions {
            builder = builder.with_installed_extension(extension);
        }

        Ok(CustomerAppRuntimePlan {
            composition,
            runtime: builder.build()?,
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
        P: AuthModelPackage,
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
