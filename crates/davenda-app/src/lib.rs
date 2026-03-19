use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use davenda_auth::AuthModelPackage;
use davenda_cli::{
    CliModelError, CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus,
};
use davenda_config::PlatformConfig;
use davenda_core::{
    AdminResourceContribution, BulkOperationDefinition, CapabilityValidationError,
    CoreServiceDependency, EventSubscription, JobContract, MigrationContract, ModuleDependency,
    ModuleDependencyKind, ModuleManifest, PlatformModule, ReportDefinition, RouteSurface,
    SearchIndexContribution, validate_module_capabilities,
};
use davenda_data::{MigrationOwner, MigrationPlan};
use davenda_i18n::LocaleTag;
use davenda_runtime::{RuntimeBuildError, RuntimeBuilder, RuntimePlan};
use davenda_template::TemplateNamespace;
use davenda_wasm::{
    ContractVersion, ExtensionConfigValue, ExtensionInstallation, ExtensionPackage, WasmModelError,
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("`{field}` has invalid hostname `{value}`")]
    InvalidHostname { field: &'static str, value: String },
    #[error("module `{module}` is installed more than once")]
    DuplicateInstalledModule { module: String },
    #[error("domain `{domain}` is declared more than once")]
    DuplicateDomain { domain: String },
    #[error("theme namespace `{namespace}` is declared more than once")]
    DuplicateThemeNamespace { namespace: String },
    #[error("content model `{model_id}` is declared more than once")]
    DuplicateContentModel { model_id: String },
    #[error("content model `{model_id}` declares duplicate field `{field_id}`")]
    DuplicateContentField { model_id: String, field_id: String },
    #[error("extension `{extension_id}` is declared more than once")]
    DuplicateExtension { extension_id: String },
    #[error("customer app `{app_id}` installs extensions but no extension packages were supplied")]
    ExtensionPackagesRequired { app_id: String },
    #[error("default locale `{default_locale}` is not in the supported locale set")]
    DefaultLocaleNotSupported { default_locale: String },
    #[error("customer app `{app_id}` must declare at least one canonical domain")]
    MissingCanonicalDomain { app_id: String },
    #[error("customer app `{app_id}` does not install module `{module}`")]
    UnknownInstalledModule { app_id: String, module: String },
    #[error("module `{module}` requires installed dependency `{dependency}`")]
    MissingModuleDependency { module: String, dependency: String },
    #[error(
        "customer app `{app_id}` configures auth package `{configured}` but runtime package `{actual}` was supplied"
    )]
    AuthPackageMismatch {
        app_id: String,
        configured: String,
        actual: String,
    },
    #[error("customer app manifest `{manifest}` does not match runtime config app `{configured}`")]
    ConfigAppMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest auth package `{manifest}` does not match runtime config auth package `{configured}`"
    )]
    ConfigAuthPackageMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest default locale `{manifest}` does not match runtime config default locale `{configured}`"
    )]
    ConfigDefaultLocaleMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest supported locales `{manifest:?}` do not match runtime config supported locales `{configured:?}`"
    )]
    ConfigSupportedLocalesMismatch {
        manifest: Vec<String>,
        configured: Vec<String>,
    },
    #[error(
        "customer app canonical host `{manifest}` does not match runtime config canonical host `{configured}`"
    )]
    ConfigCanonicalHostMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest modules differ from runtime config modules; manifest-only={manifest_only:?}, config-only={configured_only:?}"
    )]
    ConfigModulesMismatch {
        manifest_only: Vec<String>,
        configured_only: Vec<String>,
    },
    #[error("runtime provided modules not installed by customer app `{app_id}`: {modules:?}")]
    UnexpectedRuntimeModules {
        app_id: String,
        modules: Vec<String>,
    },
    #[error(
        "extension `{extension_id}` is installed for customer app `{extension_customer_app}` but manifest is `{app_id}`"
    )]
    ExtensionCustomerAppMismatch {
        extension_id: String,
        extension_customer_app: String,
        app_id: String,
    },
    #[error("customer app `{app_id}` does not have a package for extension `{extension_id}`")]
    UnknownExtensionPackage {
        app_id: String,
        extension_id: String,
    },
    #[error(
        "customer extension `{extension_id}` pins version `{configured}` but package provides `{actual}`"
    )]
    ExtensionVersionMismatch {
        extension_id: String,
        configured: ContractVersion,
        actual: ContractVersion,
    },
    #[error(
        "customer extension `{extension_id}` pins artifact digest `{configured}` but package provides `{actual}`"
    )]
    ExtensionArtifactChecksumMismatch {
        extension_id: String,
        configured: String,
        actual: String,
    },
    #[error("{0}")]
    ModuleCapabilityValidation(#[from] CapabilityValidationError),
    #[error("{0}")]
    Cli(#[from] CliModelError),
    #[error("{0}")]
    Wasm(#[from] WasmModelError),
    #[error("{message}")]
    RuntimeBuild { message: String },
}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AppModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(CustomerAppId, "customer_app_id");
token_type!(ThemeId, "theme_id");
token_type!(ContentModelId, "content_model_id");
token_type!(ContentFieldId, "content_field_id");
token_type!(ExtensionId, "extension_id");
token_type!(ModuleId, "module_id");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDomain {
    pub hostname: String,
    pub canonical: bool,
}

impl AppDomain {
    pub fn new(hostname: impl Into<String>, canonical: bool) -> Result<Self, AppModelError> {
        Ok(Self {
            hostname: validate_hostname("domain_hostname", hostname.into())?,
            canonical,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModuleSpec {
    pub id: ModuleId,
    pub version_req: Option<String>,
}

impl InstalledModuleSpec {
    pub fn new(id: impl Into<String>) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ModuleId::new(id.into())?,
            version_req: None,
        })
    }

    pub fn pinned(mut self, version_req: impl Into<String>) -> Result<Self, AppModelError> {
        self.version_req = Some(require_non_empty("module_version_req", version_req.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeProfile {
    pub active: ThemeId,
    pub template_namespaces: Vec<TemplateNamespace>,
    pub asset_roots: Vec<String>,
}

impl ThemeProfile {
    pub fn new(
        active: ThemeId,
        template_namespaces: Vec<TemplateNamespace>,
    ) -> Result<Self, AppModelError> {
        if template_namespaces.is_empty() {
            return Err(AppModelError::EmptyField {
                field: "template_namespaces",
            });
        }

        let mut seen = BTreeSet::new();
        for namespace in &template_namespaces {
            if !seen.insert(namespace.to_string()) {
                return Err(AppModelError::DuplicateThemeNamespace {
                    namespace: namespace.to_string(),
                });
            }
        }

        Ok(Self {
            active,
            template_namespaces,
            asset_roots: Vec::new(),
        })
    }

    pub fn with_asset_root(mut self, asset_root: impl Into<String>) -> Result<Self, AppModelError> {
        self.asset_roots
            .push(require_non_empty("theme_asset_root", asset_root.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFieldType {
    Text,
    RichText,
    Slug,
    Boolean,
    Integer,
    DateTime,
    Asset,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentField {
    pub id: ContentFieldId,
    pub field_type: ContentFieldType,
    pub localized: bool,
    pub required: bool,
}

impl ContentField {
    pub fn new(id: impl Into<String>, field_type: ContentFieldType) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ContentFieldId::new(id.into())?,
            field_type,
            localized: false,
            required: false,
        })
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentModel {
    pub id: ContentModelId,
    pub resource_kind: String,
    pub fields: Vec<ContentField>,
}

impl ContentModel {
    pub fn new(
        id: impl Into<String>,
        resource_kind: impl Into<String>,
        fields: Vec<ContentField>,
    ) -> Result<Self, AppModelError> {
        if fields.is_empty() {
            return Err(AppModelError::EmptyField {
                field: "content_model_fields",
            });
        }

        let id = ContentModelId::new(id.into())?;
        let resource_kind = require_non_empty("content_model_resource_kind", resource_kind.into())?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            if !seen.insert(field.id.to_string()) {
                return Err(AppModelError::DuplicateContentField {
                    model_id: id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            resource_kind,
            fields,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Extend,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStrategy {
    pub mode: AuthMode,
    pub package_name: String,
}

impl AuthStrategy {
    pub fn new(mode: AuthMode, package_name: impl Into<String>) -> Result<Self, AppModelError> {
        Ok(Self {
            mode,
            package_name: require_non_empty("auth_package_name", package_name.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerExtension {
    pub id: ExtensionId,
    pub package_version: ContractVersion,
    pub artifact_sha256: String,
    pub config: BTreeMap<String, ExtensionConfigValue>,
    pub installation: ExtensionInstallation,
}

impl CustomerExtension {
    pub fn new(
        id: impl Into<String>,
        package_version: ContractVersion,
        artifact_sha256: impl Into<String>,
        installation: ExtensionInstallation,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ExtensionId::new(id.into())?,
            package_version,
            artifact_sha256: validate_sha256("extension_artifact_sha256", artifact_sha256.into())?,
            config: BTreeMap::new(),
            installation,
        })
    }

    pub fn with_config_value(
        mut self,
        key: impl Into<String>,
        value: ExtensionConfigValue,
    ) -> Result<Self, AppModelError> {
        let key = validate_token("extension_config_key", key.into())?;
        self.config.insert(key, value);
        Ok(self)
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppComposition {
    pub app_id: CustomerAppId,
    pub display_name: String,
    pub domains: Vec<AppDomain>,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub installed_modules: Vec<InstalledModuleSpec>,
    pub module_inventory: Vec<InstalledModuleSummary>,
    pub required_core_services: Vec<CoreServiceDependency>,
    pub migrations: Vec<MigrationContract>,
    pub route_surfaces: Vec<RouteSurface>,
    pub jobs: Vec<JobContract>,
    pub event_subscriptions: Vec<EventSubscription>,
    pub admin_resources: Vec<AdminResourceContribution>,
    pub search_contributions: Vec<SearchIndexContribution>,
    pub report_definitions: Vec<ReportDefinition>,
    pub bulk_operations: Vec<BulkOperationDefinition>,
    pub theme: ThemeProfile,
    pub content_models: Vec<ContentModel>,
    pub extensions: Vec<CustomerExtension>,
    pub auth: AuthStrategy,
}

#[derive(Debug, Clone)]
pub struct CustomerAppRuntimePlan {
    pub composition: CustomerAppComposition,
    pub runtime: RuntimePlan,
    pub migration_summary: MigrationPlanSummary,
    pub release_doctor: ReleaseDoctorReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModuleSummary {
    pub id: ModuleId,
    pub version_req: Option<String>,
    pub module_dependencies: Vec<ModuleDependency>,
    pub core_service_dependencies: Vec<CoreServiceDependency>,
    pub migrations: Vec<MigrationContract>,
    pub route_surfaces: Vec<RouteSurface>,
    pub jobs: Vec<JobContract>,
    pub event_subscriptions: Vec<EventSubscription>,
    pub admin_resources: Vec<AdminResourceContribution>,
    pub search_contributions: Vec<SearchIndexContribution>,
    pub report_definitions: Vec<ReportDefinition>,
    pub bulk_operations: Vec<BulkOperationDefinition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationPlanOwner {
    Module(String),
    AuthPackage(String),
    CustomerApp(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlanEntry {
    pub owner: MigrationPlanOwner,
    pub step_id: Option<String>,
    pub order: u32,
    pub description: String,
    pub online_safe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationPlanSummary {
    entries: Vec<MigrationPlanEntry>,
}

impl MigrationPlanSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, entry: MigrationPlanEntry) {
        self.entries.push(entry);
        self.entries.sort_by(|left, right| {
            migration_owner_rank(&left.owner)
                .cmp(&migration_owner_rank(&right.owner))
                .then(left.order.cmp(&right.order))
                .then(
                    left.step_id
                        .as_deref()
                        .unwrap_or("")
                        .cmp(right.step_id.as_deref().unwrap_or("")),
                )
        });
    }

    pub fn entries(&self) -> &[MigrationPlanEntry] {
        &self.entries
    }

    pub fn command_report(&self) -> Result<CommandReport, AppModelError> {
        let mut report = CommandReport::new(
            ["migrate", "plan"],
            "Composed module, auth-package, and customer-app migration plan",
        )?
        .with_columns(["owner", "step", "order", "online_safe", "description"])?;
        if self.entries.iter().any(|entry| !entry.online_safe) {
            report = report.with_status(ReportStatus::Warning);
        }

        for entry in &self.entries {
            report.push_row(
                ReportRow::new()
                    .with_cell("owner", migration_owner_label(&entry.owner))?
                    .with_cell(
                        "step",
                        entry
                            .step_id
                            .clone()
                            .unwrap_or_else(|| "version-check".to_string()),
                    )?
                    .with_cell("order", entry.order.to_string())?
                    .with_cell("online_safe", entry.online_safe.to_string())?
                    .with_cell("description", entry.description.clone())?,
            );
        }

        Ok(report)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseDoctorSeverity {
    Info,
    Warning,
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDoctorFinding {
    pub severity: ReleaseDoctorSeverity,
    pub code: String,
    pub message: String,
}

impl ReleaseDoctorFinding {
    pub fn new(
        severity: ReleaseDoctorSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDoctorReport {
    pub app_id: CustomerAppId,
    pub findings: Vec<ReleaseDoctorFinding>,
}

impl ReleaseDoctorReport {
    pub fn is_compatible(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.severity == ReleaseDoctorSeverity::Blocking)
    }

    pub fn blocking_findings(&self) -> impl Iterator<Item = &ReleaseDoctorFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.severity == ReleaseDoctorSeverity::Blocking)
    }

    pub fn command_report(&self) -> Result<CommandReport, AppModelError> {
        let mut report = CommandReport::new(
            ["release", "doctor"],
            format!(
                "Checked upgrade compatibility for customer app `{}`",
                self.app_id
            ),
        )?
        .with_columns(["severity", "code", "message"])?;
        report = report.with_status(
            match self
                .findings
                .iter()
                .map(|finding| finding.severity)
                .max_by_key(|severity| release_doctor_rank(*severity))
            {
                Some(ReleaseDoctorSeverity::Blocking) => ReportStatus::Unsafe,
                Some(ReleaseDoctorSeverity::Warning) => ReportStatus::Warning,
                _ => ReportStatus::Ok,
            },
        );

        for finding in &self.findings {
            report.push_row(
                ReportRow::new()
                    .with_cell("severity", release_doctor_label(finding.severity))?
                    .with_cell("code", finding.code.clone())?
                    .with_cell("message", finding.message.clone())?,
            );
            report.push_diagnostic(DiagnosticRecord::new(
                match finding.severity {
                    ReleaseDoctorSeverity::Info => DiagnosticSeverity::Info,
                    ReleaseDoctorSeverity::Warning => DiagnosticSeverity::Warning,
                    ReleaseDoctorSeverity::Blocking => DiagnosticSeverity::Error,
                },
                finding.code.clone(),
                finding.message.clone(),
            )?);
        }

        Ok(report)
    }
}

impl CustomerAppComposition {
    pub fn module_list(&self) -> &[InstalledModuleSummary] {
        &self.module_inventory
    }

    pub fn canonical_domain(&self) -> Option<&str> {
        self.domains
            .iter()
            .find(|domain| domain.canonical)
            .map(|domain| domain.hostname.as_str())
    }

    pub fn release_doctor(&self, config: Option<&PlatformConfig>) -> ReleaseDoctorReport {
        let mut findings = Vec::new();
        let installed_modules = self
            .installed_modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>();

        for module in &self.module_inventory {
            if module.version_req.is_none() {
                findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Warning,
                    "module.version.unpinned",
                    format!(
                        "official module `{}` is not version pinned in the customer app manifest",
                        module.id
                    ),
                ));
            }
        }

        if self.theme.asset_roots.is_empty() {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Warning,
                "theme.assets.missing",
                "the active theme declares no asset roots, so asset publication will be a no-op",
            ));
        }

        if !self.admin_resources.is_empty()
            && !installed_modules.iter().any(|module| module == "admin")
        {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "module.admin.missing",
                "admin resources are composed into the customer app but the `admin` module is not installed",
            ));
        }

        if (!self.search_contributions.is_empty()
            || !self.report_definitions.is_empty()
            || !self.bulk_operations.is_empty())
            && !installed_modules.iter().any(|module| module == "ops")
        {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "module.ops.missing",
                "search, reporting, or bulk-operation contracts are present but the `ops` module is not installed",
            ));
        }

        if let Some(config) = config {
            findings.extend(config_alignment_findings(self, config));

            if !config.assets.publish_manifest && !self.theme.asset_roots.is_empty() {
                findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Warning,
                    "assets.publish.disabled",
                    "theme assets are declared but `assets.publish_manifest` is disabled in config",
                ));
            }
        }

        ReleaseDoctorReport {
            app_id: self.app_id.clone(),
            findings,
        }
    }

    pub fn module_list_report(&self) -> Result<CommandReport, AppModelError> {
        let mut report = CommandReport::new(
            ["module", "list"],
            format!("Installed modules for customer app `{}`", self.app_id),
        )?
        .with_columns([
            "module",
            "version",
            "core_services",
            "module_dependencies",
            "routes",
            "jobs",
            "admin_resources",
        ])?;

        if self
            .module_inventory
            .iter()
            .any(|module| module.version_req.is_none())
        {
            report = report.with_status(ReportStatus::Warning);
        }

        for module in &self.module_inventory {
            report.push_row(
                ReportRow::new()
                    .with_cell("module", module.id.to_string())?
                    .with_cell(
                        "version",
                        module
                            .version_req
                            .clone()
                            .unwrap_or_else(|| "unpinned".to_string()),
                    )?
                    .with_cell(
                        "core_services",
                        join_display(
                            module
                                .core_service_dependencies
                                .iter()
                                .map(|dependency| format!("{dependency:?}")),
                        ),
                    )?
                    .with_cell(
                        "module_dependencies",
                        if module.module_dependencies.is_empty() {
                            "none".to_string()
                        } else {
                            module
                                .module_dependencies
                                .iter()
                                .map(|dependency| dependency.module.clone())
                                .collect::<Vec<_>>()
                                .join(",")
                        },
                    )?
                    .with_cell("routes", module.route_surfaces.len().to_string())?
                    .with_cell("jobs", module.jobs.len().to_string())?
                    .with_cell("admin_resources", module.admin_resources.len().to_string())?,
            );
        }

        Ok(report)
    }
}

impl From<RuntimeBuildError> for AppModelError {
    fn from(error: RuntimeBuildError) -> Self {
        Self::RuntimeBuild {
            message: error.to_string(),
        }
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(AppModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn validate_hostname(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        && trimmed.contains('.')
    {
        Ok(trimmed)
    } else {
        Err(AppModelError::InvalidHostname {
            field,
            value: trimmed,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_sha256(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed.len() == 64
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        Ok(trimmed)
    } else {
        Err(AppModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

fn build_migration_summary(
    manifest: &CustomerAppManifest,
    auth_package_name: String,
    modules: &[Box<dyn PlatformModule>],
) -> MigrationPlanSummary {
    let mut summary = MigrationPlanSummary::new();
    let installed_modules = manifest
        .modules
        .iter()
        .map(|module| module.id.to_string())
        .collect::<BTreeSet<_>>();

    for module in modules {
        let module_manifest = module.manifest();
        if !installed_modules.contains(&module_manifest.name) {
            continue;
        }

        if let Some(plan) = module.install_migration_plan() {
            append_migration_plan(&mut summary, &plan);
        }
    }

    summary.push(MigrationPlanEntry {
        owner: MigrationPlanOwner::AuthPackage(auth_package_name.clone()),
        step_id: None,
        order: 0,
        description: format!(
            "validate auth package `{auth_package_name}` schema, model, and capability bindings before release"
        ),
        online_safe: true,
    });

    for migration in &manifest.customer_migrations {
        summary.push(MigrationPlanEntry {
            owner: MigrationPlanOwner::CustomerApp(manifest.id.to_string()),
            step_id: None,
            order: migration.order,
            description: migration.description.clone(),
            online_safe: true,
        });
    }

    summary
}

fn append_migration_plan(summary: &mut MigrationPlanSummary, plan: &MigrationPlan) {
    for step in plan.ordered_steps() {
        let owner = match &step.owner {
            MigrationOwner::Module(module) => MigrationPlanOwner::Module(module.clone()),
            MigrationOwner::AuthPackage(package) => {
                MigrationPlanOwner::AuthPackage(package.clone())
            }
            MigrationOwner::CustomerApp(app_id) => MigrationPlanOwner::CustomerApp(app_id.clone()),
            MigrationOwner::Core => continue,
        };

        summary.push(MigrationPlanEntry {
            owner,
            step_id: Some(step.id.to_string()),
            order: step.order,
            description: step.description.clone(),
            online_safe: step.online_safe,
        });
    }
}

fn config_alignment_findings(
    composition: &CustomerAppComposition,
    config: &PlatformConfig,
) -> Vec<ReleaseDoctorFinding> {
    let mut findings = Vec::new();

    if config.app.name != composition.app_id.as_str() {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.app.mismatch",
            format!(
                "runtime config app `{}` does not match customer app manifest `{}`",
                config.app.name, composition.app_id
            ),
        ));
    }

    if config.auth.package != composition.auth.package_name {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.auth_package.mismatch",
            format!(
                "runtime config auth package `{}` does not match customer app auth package `{}`",
                config.auth.package, composition.auth.package_name
            ),
        ));
    }

    if config.i18n.default_locale != composition.default_locale.as_str() {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.i18n.default_locale",
            format!(
                "runtime config default locale `{}` does not match customer app default locale `{}`",
                config.i18n.default_locale, composition.default_locale
            ),
        ));
    }

    let manifest_locales = sorted_locale_strings(&composition.supported_locales);
    let configured_locales = sorted_strings(config.i18n.supported_locales.clone());
    if manifest_locales != configured_locales {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.i18n.supported_locales",
            format!(
                "runtime config supported locales {:?} do not match customer app locales {:?}",
                configured_locales, manifest_locales
            ),
        ));
    }

    if let Some(canonical_domain) = composition.canonical_domain() {
        if config.seo.canonical_host != canonical_domain {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "config.seo.canonical_host",
                format!(
                    "runtime config canonical host `{}` does not match customer app canonical domain `{}`",
                    config.seo.canonical_host, canonical_domain
                ),
            ));
        }
    }

    let manifest_modules = sorted_strings(
        composition
            .installed_modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>(),
    );
    let configured_modules = sorted_strings(config.modules.enabled.clone());
    let manifest_only = difference(&manifest_modules, &configured_modules);
    let configured_only = difference(&configured_modules, &manifest_modules);
    if !manifest_only.is_empty() || !configured_only.is_empty() {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.modules.enabled",
            format!(
                "runtime config modules drift from the customer app manifest; manifest-only={manifest_only:?}, config-only={configured_only:?}"
            ),
        ));
    }

    findings
}

fn migration_owner_rank(owner: &MigrationPlanOwner) -> u8 {
    match owner {
        MigrationPlanOwner::Module(_) => 1,
        MigrationPlanOwner::AuthPackage(_) => 2,
        MigrationPlanOwner::CustomerApp(_) => 3,
    }
}

fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn sorted_locale_strings(locales: &[LocaleTag]) -> Vec<String> {
    sorted_strings(locales.iter().map(ToString::to_string).collect())
}

fn difference(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|value| !right.contains(value))
        .cloned()
        .collect()
}

fn join_display<T>(values: impl IntoIterator<Item = T>) -> String
where
    T: fmt::Display,
{
    let rendered = values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        "none".to_string()
    } else {
        rendered.join(",")
    }
}

fn migration_owner_label(owner: &MigrationPlanOwner) -> String {
    match owner {
        MigrationPlanOwner::Module(module) => format!("module:{module}"),
        MigrationPlanOwner::AuthPackage(package) => format!("auth:{package}"),
        MigrationPlanOwner::CustomerApp(app_id) => format!("customer_app:{app_id}"),
    }
}

fn release_doctor_rank(severity: ReleaseDoctorSeverity) -> u8 {
    match severity {
        ReleaseDoctorSeverity::Info => 0,
        ReleaseDoctorSeverity::Warning => 1,
        ReleaseDoctorSeverity::Blocking => 2,
    }
}

fn release_doctor_label(severity: ReleaseDoctorSeverity) -> &'static str {
    match severity {
        ReleaseDoctorSeverity::Info => "info",
        ReleaseDoctorSeverity::Warning => "warning",
        ReleaseDoctorSeverity::Blocking => "blocking",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use davenda_auth::{Capability, DefaultAuthModelPackage};
    use davenda_config::PlatformConfig;
    use davenda_core::{
        AdminContributionKind, AdminNavigationSection, BulkOperationKind, BulkOperationScope,
        CapabilityContract, ReportDeliveryMode, ReportFormat, ReportSensitivity,
        SearchDocumentKind, SearchFieldContribution, SearchFieldRole, SearchIndexContribution,
        SearchInvalidationRule, SearchInvalidationTrigger, SearchRebuildStrategy, SearchVisibility,
    };
    use davenda_data::{
        MigrationId, MigrationOwner as DataMigrationOwner, MigrationPlan, MigrationStep,
    };
    use davenda_wasm::{
        ContractVersion, ExtensionArtifactSource, ExtensionConfigField, ExtensionConfigSchema,
        ExtensionConfigValue, ExtensionConfigValueType, ExtensionManifest, ExtensionPackage,
        ExtensionPoint, HandlerId, HandlerInstallation, HandlerManifest, HostCapabilityGrant,
        HostGrantSet, RenderHookExtensionPoint, ResourceLimits,
    };

    fn locale(value: &str) -> LocaleTag {
        LocaleTag::new(value).expect("locale is valid")
    }

    fn theme() -> ThemeProfile {
        ThemeProfile::new(
            ThemeId::new("harbor").unwrap(),
            vec![
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateNamespace::new("harbor").unwrap(),
            ],
        )
        .unwrap()
        .with_asset_root("theme/assets")
        .unwrap()
    }

    fn auth() -> AuthStrategy {
        AuthStrategy::new(
            AuthMode::Extend,
            DefaultAuthModelPackage::default().manifest().name.clone(),
        )
        .unwrap()
    }

    fn app() -> CustomerAppManifest {
        CustomerAppManifest::new(
            CustomerAppId::new("harbor-shop").unwrap(),
            "Harbor Shop",
            locale("en-GB"),
            vec![locale("en-GB"), locale("fr-FR")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("shop.example.com", true).unwrap())
        .with_module(InstalledModuleSpec::new("cms").unwrap())
        .with_module(InstalledModuleSpec::new("commerce").unwrap())
        .with_content_model(
            ContentModel::new(
                "landing_page",
                "page",
                vec![
                    ContentField::new("title", ContentFieldType::Text)
                        .unwrap()
                        .localized()
                        .required(),
                    ContentField::new("hero_image", ContentFieldType::Asset).unwrap(),
                ],
            )
            .unwrap(),
        )
        .with_customer_migration(MigrationContract::new(
            "customer.content",
            90,
            "Creates customer app landing-page projections",
        ))
        .with_extension(
            CustomerExtension::new(
                "loyalty-widget",
                ContractVersion::new(1, 2, 3),
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                ExtensionInstallation::new(
                    "harbor-shop",
                    vec![HandlerInstallation::new(
                        HandlerId::new("account.loyalty.widget").unwrap(),
                        HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                            slot: "cms.page.render".to_string(),
                        }]),
                    )],
                )
                .unwrap(),
            )
            .unwrap()
            .with_config_value(
                "program_slug",
                ExtensionConfigValue::String("harbor-club".to_string()),
            )
            .unwrap(),
        )
    }

    fn extension_package() -> ExtensionPackage {
        ExtensionPackage::new(
            "worka",
            ExtensionManifest::new(
                davenda_wasm::ExtensionId::new("loyalty-widget").unwrap(),
                "Loyalty Widget",
                ContractVersion::new(1, 2, 3),
                ContractVersion::new(1, 0, 0),
                ResourceLimits::baseline_for(davenda_wasm::ExtensionPointKind::RenderHook),
                vec![
                    HandlerManifest::new(
                        HandlerId::new("account.loyalty.widget").unwrap(),
                        "exports.loyalty_widget",
                        ExtensionPoint::RenderHook(
                            RenderHookExtensionPoint::new("cms.page.render").unwrap(),
                        ),
                        HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                            slot: "cms.page.render".to_string(),
                        }]),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
            ExtensionArtifactSource::local_path("extensions/loyalty-widget.wasm").unwrap(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ExtensionConfigSchema::new(
                1,
                vec![
                    ExtensionConfigField::required(
                        "program_slug",
                        ExtensionConfigValueType::String,
                    )
                    .unwrap(),
                    ExtensionConfigField::optional(
                        "show_points",
                        ExtensionConfigValueType::Boolean,
                    )
                    .unwrap()
                    .with_default(ExtensionConfigValue::Boolean(true))
                    .unwrap(),
                ],
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn module_manifests() -> Vec<ModuleManifest> {
        vec![
            ModuleManifest::new("cms")
                .with_optional_capabilities(vec![Capability::CmsPagePublish])
                .with_capability_contracts(vec![CapabilityContract::optional(
                    Capability::CmsPagePublish,
                    ["page"],
                )])
                .with_core_service_dependencies(vec![CoreServiceDependency::Seo])
                .with_migrations(vec![MigrationContract::new(
                    "cms.pages",
                    10,
                    "Creates CMS page storage",
                )])
                .with_route_surfaces(vec![RouteSurface::new(
                    "cms.page",
                    davenda_core::RouteSurfaceKind::FrontendPage,
                    "/pages/{slug}",
                )])
                .with_jobs(vec![JobContract::new(
                    "cms.publish-scheduled",
                    davenda_core::JobTriggerKind::Scheduled,
                    true,
                    "Publishes scheduled pages",
                )])
                .with_event_subscriptions(vec![EventSubscription::new(
                    "cms.page.publish-requested",
                    Some("cms.publish-scheduled"),
                    "Schedules future publication work",
                )])
                .with_admin_resources(vec![AdminResourceContribution::new(
                    "cms.pages",
                    "/admin/pages",
                    "Pages",
                    "Pages",
                    AdminNavigationSection::Content,
                    AdminContributionKind::ResourceIndex,
                    Capability::CmsPagePublish,
                )])
                .with_extension_slots(vec![davenda_core::ExtensionSlotDescriptor::new(
                    davenda_core::ExtensionSlotKind::RenderHook,
                    "cms.page.render",
                    "Allows render-hook extensions to augment CMS page rendering",
                )])
                .with_search_contributions(vec![SearchIndexContribution::new(
                    "search.cms.pages",
                    SearchDocumentKind::Page,
                    SearchVisibility::Public,
                    true,
                    vec![SearchFieldContribution::new(
                        "title",
                        "title",
                        SearchFieldRole::Title,
                        true,
                        true,
                    )],
                    vec![SearchInvalidationRule::new(
                        SearchInvalidationTrigger::Published,
                        "page published",
                    )],
                    SearchRebuildStrategy::OnInvalidate,
                )]),
            ModuleManifest::new("commerce")
                .with_optional_capabilities(vec![Capability::OrderRead])
                .with_capability_contracts(vec![CapabilityContract::optional(
                    Capability::OrderRead,
                    ["order"],
                )])
                .with_module_dependencies(vec![davenda_core::ModuleDependency::required(
                    "cms",
                    "Commerce storefront installs depend on CMS navigation and content surfaces",
                )])
                .with_core_service_dependencies(vec![CoreServiceDependency::Jobs])
                .with_report_definitions(vec![ReportDefinition::new(
                    "report.orders.summary",
                    "Orders summary",
                    Some("Operational order summary".to_string()),
                    Capability::OrderRead,
                    ReportFormat::Csv,
                    ReportSensitivity::Restricted,
                    ReportDeliveryMode::InternalOnly,
                    "reports/orders",
                    davenda_jobs::RetryPolicy::new(
                        3,
                        std::time::Duration::from_secs(15),
                        std::time::Duration::from_secs(300),
                    )
                    .unwrap(),
                )])
                .with_bulk_operations(vec![BulkOperationDefinition::new(
                    "bulk.orders.export",
                    "Bulk export orders",
                    Some("Queues order exports".to_string()),
                    Capability::OrderRead,
                    BulkOperationKind::Export,
                    BulkOperationScope::Commerce,
                    davenda_jobs::RetryPolicy::new(
                        3,
                        std::time::Duration::from_secs(15),
                        std::time::Duration::from_secs(300),
                    )
                    .unwrap(),
                    Some(100),
                    true,
                )]),
        ]
    }

    #[derive(Debug)]
    struct StaticModule {
        manifest: ModuleManifest,
        migration_plan: Option<MigrationPlan>,
    }

    impl StaticModule {
        fn new(manifest: ModuleManifest) -> Self {
            let migration_plan = match manifest.name.as_str() {
                "cms" => Some(static_migration_plan("cms", "001_pages")),
                "commerce" => Some(static_migration_plan("commerce", "001_catalog")),
                _ => None,
            };

            Self {
                manifest,
                migration_plan,
            }
        }
    }

    impl PlatformModule for StaticModule {
        fn manifest(&self) -> ModuleManifest {
            self.manifest.clone()
        }

        fn register(
            &self,
            _registry: &mut davenda_core::ServiceRegistry,
        ) -> Result<(), davenda_core::RegistrationError> {
            Ok(())
        }

        fn install_migration_plan(&self) -> Option<MigrationPlan> {
            self.migration_plan.clone()
        }
    }

    fn static_migration_plan(module: &str, step_id: &str) -> MigrationPlan {
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new(step_id).unwrap(),
                DataMigrationOwner::Module(module.to_string()),
                10,
                format!("install {module} tables"),
            )
            .unwrap()
            .with_statement("SELECT 1")
            .unwrap(),
        )
        .unwrap();
        plan
    }

    fn runtime_config(app_id: &str) -> PlatformConfig {
        PlatformConfig::from_toml_str(&format!(
            r#"
[app]
name = "{app_id}"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
object_store = "s3"
local_root = "/var/lib/platform"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "shop.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false

[modules]
enabled = ["cms", "commerce"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#
        ))
        .expect("runtime config is valid")
    }

    #[test]
    fn manifest_requires_supported_default_locale_and_canonical_domain() {
        let invalid = CustomerAppManifest::new(
            CustomerAppId::new("invalid").unwrap(),
            "Invalid",
            locale("en-GB"),
            vec![locale("fr-FR")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("preview.example.com", false).unwrap());

        assert_eq!(
            invalid.validate().unwrap_err(),
            AppModelError::DefaultLocaleNotSupported {
                default_locale: "en-GB".to_string(),
            }
        );
    }

    #[test]
    fn manifest_rejects_duplicate_modules_and_extension_app_mismatch() {
        let duplicated = app().with_module(InstalledModuleSpec::new("cms").unwrap());
        assert_eq!(
            duplicated.validate().unwrap_err(),
            AppModelError::DuplicateInstalledModule {
                module: "cms".to_string(),
            }
        );

        let mismatched = CustomerAppManifest::new(
            CustomerAppId::new("mismatch").unwrap(),
            "Mismatch",
            locale("en-GB"),
            vec![locale("en-GB")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("mismatch.example.com", true).unwrap())
        .with_extension(
            CustomerExtension::new(
                "widget",
                ContractVersion::new(1, 0, 0),
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                ExtensionInstallation::new("other-app", Vec::new()).unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(
            mismatched.validate().unwrap_err(),
            AppModelError::ExtensionCustomerAppMismatch {
                extension_id: "widget".to_string(),
                extension_customer_app: "other-app".to_string(),
                app_id: "mismatch".to_string(),
            }
        );
    }

    #[test]
    fn composition_collects_installed_module_contracts() {
        let composition = app()
            .compose(&DefaultAuthModelPackage::default(), &module_manifests())
            .unwrap();

        assert_eq!(composition.installed_modules.len(), 2);
        assert_eq!(composition.module_list().len(), 2);
        assert_eq!(composition.route_surfaces.len(), 1);
        assert_eq!(composition.jobs.len(), 1);
        assert_eq!(composition.event_subscriptions.len(), 1);
        assert_eq!(composition.admin_resources.len(), 1);
        assert_eq!(composition.search_contributions.len(), 1);
        assert_eq!(composition.report_definitions.len(), 1);
        assert_eq!(composition.bulk_operations.len(), 1);
        assert_eq!(composition.migrations.len(), 2);
        assert_eq!(composition.canonical_domain(), Some("shop.example.com"));
        assert!(
            composition
                .required_core_services
                .contains(&CoreServiceDependency::Seo)
        );
        assert!(
            composition
                .required_core_services
                .contains(&CoreServiceDependency::Jobs)
        );
        assert_eq!(
            composition.module_list()[0].id,
            ModuleId::new("cms").unwrap()
        );
        assert_eq!(
            composition.module_list()[1].module_dependencies[0].module,
            "cms".to_string()
        );
    }

    #[test]
    fn composition_rejects_unknown_modules_and_missing_dependencies() {
        let unknown = CustomerAppManifest::new(
            CustomerAppId::new("unknown").unwrap(),
            "Unknown",
            locale("en-GB"),
            vec![locale("en-GB")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("unknown.example.com", true).unwrap())
        .with_module(InstalledModuleSpec::new("events").unwrap());

        assert_eq!(
            unknown
                .compose(&DefaultAuthModelPackage::default(), &module_manifests())
                .unwrap_err(),
            AppModelError::UnknownInstalledModule {
                app_id: "unknown".to_string(),
                module: "events".to_string(),
            }
        );

        let missing_dependency = CustomerAppManifest::new(
            CustomerAppId::new("dependency").unwrap(),
            "Dependency",
            locale("en-GB"),
            vec![locale("en-GB")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("dependency.example.com", true).unwrap())
        .with_module(InstalledModuleSpec::new("commerce").unwrap());

        assert_eq!(
            missing_dependency
                .compose(&DefaultAuthModelPackage::default(), &module_manifests())
                .unwrap_err(),
            AppModelError::MissingModuleDependency {
                module: "commerce".to_string(),
                dependency: "cms".to_string(),
            }
        );
    }

    #[test]
    fn customer_app_can_build_a_runtime_plan_from_selected_modules() {
        let runtime = app()
            .build_runtime_plan_with_extensions(
                runtime_config("harbor-shop"),
                DefaultAuthModelPackage::default(),
                module_manifests()
                    .into_iter()
                    .map(StaticModule::new)
                    .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                    .collect(),
                vec![extension_package()],
            )
            .unwrap();

        assert_eq!(
            runtime.composition.app_id,
            CustomerAppId::new("harbor-shop").unwrap()
        );
        assert_eq!(runtime.runtime.config.app.name, "harbor-shop");
        assert_eq!(runtime.runtime.modules.len(), 2);
        assert_eq!(runtime.migration_summary.entries().len(), 4);
        assert!(runtime
            .migration_summary
            .entries()
            .iter()
            .any(|entry| matches!(
                entry.owner,
                MigrationPlanOwner::AuthPackage(ref package) if package == "platform-default-auth"
            )));
        assert!(
            runtime
                .migration_summary
                .entries()
                .iter()
                .any(|entry| matches!(
                    entry.owner,
                    MigrationPlanOwner::CustomerApp(ref app_id) if app_id == "harbor-shop"
                ))
        );
        assert!(!runtime.release_doctor.is_compatible());
        assert!(
            runtime
                .release_doctor
                .findings
                .iter()
                .any(|finding| finding.code == "module.ops.missing")
        );
    }

    #[test]
    fn runtime_build_requires_pinned_extension_packages() {
        let error = app()
            .build_runtime_plan(
                runtime_config("harbor-shop"),
                DefaultAuthModelPackage::default(),
                module_manifests()
                    .into_iter()
                    .map(StaticModule::new)
                    .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                    .collect(),
            )
            .unwrap_err();

        assert_eq!(
            error,
            AppModelError::ExtensionPackagesRequired {
                app_id: "harbor-shop".to_string(),
            }
        );

        let mut wrong_checksum = extension_package();
        wrong_checksum.artifact_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let error = app()
            .build_runtime_plan_with_extensions(
                runtime_config("harbor-shop"),
                DefaultAuthModelPackage::default(),
                module_manifests()
                    .into_iter()
                    .map(StaticModule::new)
                    .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                    .collect(),
                vec![wrong_checksum],
            )
            .unwrap_err();

        assert_eq!(
            error,
            AppModelError::ExtensionArtifactChecksumMismatch {
                extension_id: "loyalty-widget".to_string(),
                configured: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
                actual: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .to_string(),
            }
        );
    }

    #[test]
    fn runtime_build_rejects_config_module_drift_and_unexpected_runtime_modules() {
        let mut drifted = runtime_config("harbor-shop");
        drifted.modules.enabled.push("events".to_string());

        assert_eq!(
            app()
                .build_runtime_plan(
                    drifted,
                    DefaultAuthModelPackage::default(),
                    module_manifests()
                        .into_iter()
                        .map(StaticModule::new)
                        .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                        .collect(),
                )
                .unwrap_err(),
            AppModelError::ConfigModulesMismatch {
                manifest_only: Vec::new(),
                configured_only: vec!["events".to_string()],
            }
        );

        let mut modules = module_manifests()
            .into_iter()
            .map(StaticModule::new)
            .map(|module| Box::new(module) as Box<dyn PlatformModule>)
            .collect::<Vec<_>>();
        modules.push(Box::new(StaticModule::new(ModuleManifest::new("media"))));

        assert_eq!(
            app()
                .build_runtime_plan(
                    runtime_config("harbor-shop"),
                    DefaultAuthModelPackage::default(),
                    modules,
                )
                .unwrap_err(),
            AppModelError::UnexpectedRuntimeModules {
                app_id: "harbor-shop".to_string(),
                modules: vec!["media".to_string()],
            }
        );
    }

    #[test]
    fn release_doctor_reports_config_drift_and_unpinned_modules() {
        let manifest = app();
        let composition = manifest
            .compose(&DefaultAuthModelPackage::default(), &module_manifests())
            .unwrap();
        let report = composition.release_doctor(Some(&runtime_config("harbor-shop")));

        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "module.version.unpinned")
        );
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "module.ops.missing")
        );

        let mut drifted = runtime_config("harbor-shop");
        drifted.seo.canonical_host = "preview.example.com".to_string();
        let report = composition.release_doctor(Some(&drifted));
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "config.seo.canonical_host")
        );

        let mut wrong_checksum = extension_package();
        wrong_checksum.artifact_sha256 =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let report = manifest
            .release_doctor_with_extensions(
                &DefaultAuthModelPackage::default(),
                &module_manifests(),
                &[wrong_checksum],
                Some(&runtime_config("harbor-shop")),
            )
            .unwrap();
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == "extension.checksum.mismatch")
        );
    }

    #[test]
    fn customer_app_reports_render_into_cli_surfaces() {
        let runtime = app()
            .build_runtime_plan_with_extensions(
                runtime_config("harbor-shop"),
                DefaultAuthModelPackage::default(),
                module_manifests()
                    .into_iter()
                    .map(StaticModule::new)
                    .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                    .collect(),
                vec![extension_package()],
            )
            .unwrap();

        let modules = runtime.composition.module_list_report().unwrap();
        assert_eq!(
            modules.command,
            vec!["module".to_string(), "list".to_string()]
        );
        assert_eq!(modules.rows.len(), 2);

        let migrations = runtime.migration_summary.command_report().unwrap();
        assert_eq!(
            migrations.command,
            vec!["migrate".to_string(), "plan".to_string()]
        );
        assert!(migrations.rows.len() >= 4);

        let release = runtime.release_doctor.command_report().unwrap();
        assert_eq!(
            release.command,
            vec!["release".to_string(), "doctor".to_string()]
        );
        assert!(
            release
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "module.ops.missing")
        );
    }
}
