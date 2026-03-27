use super::*;
use davenda_auth::{LoadedAuthModelPackage, load_auth_model_package_at};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::path::Path;
use std::path::PathBuf;

/// Explicit runtime bootstrap entrypoint for ADR 96 customer-root binaries/workspaces.
///
/// This keeps the linked-customer composition path visible in code without forcing every
/// customer binary to start from the lower-level `RuntimeBuilder::new(...)` surface.
pub struct CustomerRootRuntimeBuilder<P> {
    inner: RuntimeBuilder<P>,
    config: PlatformConfig,
    app_root: Option<PathBuf>,
    enabled_modules: Vec<String>,
}

pub struct CustomerRootBootstrapInputs {
    pub app_root: PathBuf,
    pub manifest_path: PathBuf,
    pub enabled_modules: Vec<String>,
    pub config_path: PathBuf,
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub auth_package: LoadedAuthModelPackage,
}

#[derive(Debug, Clone, Deserialize)]
struct CustomerRootManifestDocument {
    app: CustomerRootManifestApp,
    domains: CustomerRootManifestDomains,
    i18n: CustomerRootManifestI18n,
    auth: CustomerRootManifestAuth,
    modules: CustomerRootManifestModules,
}

#[derive(Debug, Clone, Deserialize)]
struct CustomerRootManifestApp {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CustomerRootManifestDomains {
    canonical: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CustomerRootManifestI18n {
    default_locale: String,
    supported_locales: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CustomerRootManifestAuth {
    package: String,
}

#[derive(Debug, Clone, Deserialize)]
struct CustomerRootManifestModules {
    enabled: Vec<String>,
}

pub fn customer_root_runtime<P>(
    config: PlatformConfig,
    auth_package: P,
) -> CustomerRootRuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    CustomerRootRuntimeBuilder::new(config, auth_package)
}

pub fn customer_root_runtime_from_env()
-> Result<CustomerRootRuntimeBuilder<LoadedAuthModelPackage>, RuntimeBootstrapError> {
    CustomerRootRuntimeBuilder::from_env()
}

pub fn customer_root_runtime_from_paths(
    app_root: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
) -> Result<CustomerRootRuntimeBuilder<LoadedAuthModelPackage>, RuntimeBootstrapError> {
    CustomerRootRuntimeBuilder::from_paths(app_root, config_path)
}

pub fn customer_root_bootstrap_inputs_from_env()
-> Result<CustomerRootBootstrapInputs, RuntimeBootstrapError> {
    CustomerRootBootstrapInputs::from_env()
}

pub fn customer_root_bootstrap_inputs_from_paths(
    app_root: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
) -> Result<CustomerRootBootstrapInputs, RuntimeBootstrapError> {
    CustomerRootBootstrapInputs::from_paths(app_root, config_path)
}

impl<P> CustomerRootRuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            inner: RuntimeBuilder::new(config.clone(), auth_package),
            config,
            app_root: None,
            enabled_modules: Vec::new(),
        }
    }

    /// Mount a customer workspace/app root that contains the checked-in template tree.
    ///
    /// This is the customer-root equivalent of `with_template_root(...)`.
    pub fn with_customer_root<A>(mut self, root: A) -> Self
    where
        A: Into<PathBuf>,
    {
        let root = root.into();
        self.app_root = Some(root.clone());
        self.inner = self.inner.with_template_root(root);
        self
    }

    /// Link a native customer backend plugin into the runtime plan.
    pub fn with_linked_customer_plugin<C>(mut self, plugin: C) -> Self
    where
        C: CustomerBackendPlugin,
    {
        self.inner = self.inner.register_customer_plugin(plugin);
        self
    }

    pub fn with_boxed_linked_customer_plugin(
        mut self,
        plugin: Box<dyn CustomerBackendPlugin>,
    ) -> Self {
        self.inner = self.inner.with_boxed_customer_plugin(plugin);
        self
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.inner = self.inner.with_module(module);
        self
    }

    pub fn register_module<M>(self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.with_module(module)
    }

    pub fn with_boxed_module(mut self, module: Box<dyn PlatformModule>) -> Self {
        self.inner = self.inner.with_boxed_module(module);
        self
    }

    pub fn register_customer_plugin<C>(self, plugin: C) -> Self
    where
        C: CustomerBackendPlugin,
    {
        self.with_linked_customer_plugin(plugin)
    }

    pub fn with_installed_extension(mut self, extension: InstalledExtension) -> Self {
        self.inner = self.inner.with_installed_extension(extension);
        self
    }

    pub fn with_template(mut self, template: davenda_template::TemplateDefinition) -> Self {
        self.inner = self.inner.with_template(template);
        self
    }

    pub fn with_templates<I>(mut self, templates: I) -> Self
    where
        I: IntoIterator<Item = davenda_template::TemplateDefinition>,
    {
        self.inner = self.inner.with_templates(templates);
        self
    }

    pub fn with_storage_policy_rule(mut self, rule: PathPolicyRule) -> Self {
        self.inner = self.inner.with_storage_policy_rule(rule);
        self
    }

    pub fn with_storage_policies(mut self, policies: StoragePolicySet) -> Self {
        self.inner = self.inner.with_storage_policies(policies);
        self
    }

    pub fn with_route(mut self, route: RouteDefinition) -> Self {
        self.inner = self.inner.with_route(route);
        self
    }

    pub fn with_handler(mut self, handler: HandlerDefinition) -> Self {
        self.inner = self.inner.with_handler(handler);
        self
    }

    pub fn with_feature_flag(mut self, feature_flag: FeatureFlag) -> Self {
        self.inner = self.inner.with_feature_flag(feature_flag);
        self
    }

    pub fn with_maintenance_mode(mut self, maintenance_mode: MaintenanceMode) -> Self {
        self.inner = self.inner.with_maintenance_mode(maintenance_mode);
        self
    }

    /// Drop to the lower-level runtime builder when the customer binary needs advanced knobs that
    /// are not part of the customer-root convenience surface yet.
    pub fn into_runtime_builder(self) -> RuntimeBuilder<P> {
        self.inner
    }

    pub fn build(self) -> Result<RuntimePlan, RuntimeBuildError> {
        let enabled_modules = if self.enabled_modules.is_empty() {
            self.resolve_enabled_modules_from_customer_root()?
        } else {
            self.enabled_modules
        };
        self.inner
            .resolve_enabled_customer_modules(&enabled_modules)?
            .build()
    }

    pub fn run_from_env(self) -> Result<(), RuntimeBootstrapError> {
        self.build()?.serve_from_env(env::var("DAVENDA_BIND").ok())
    }
}

impl CustomerRootRuntimeBuilder<LoadedAuthModelPackage> {
    pub fn from_env() -> Result<Self, RuntimeBootstrapError> {
        let app_root = env::current_dir().map_err(RuntimeBootstrapError::CurrentDirectory)?;
        let config_path = discover_default_config_path(&app_root).ok_or_else(|| {
            RuntimeBootstrapError::ConfigNotFound {
                app_root: app_root.clone(),
            }
        })?;
        Self::from_paths(app_root, config_path)
    }

    pub fn from_paths(
        app_root: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
    ) -> Result<Self, RuntimeBootstrapError> {
        let inputs = CustomerRootBootstrapInputs::from_paths(app_root, config_path)?;
        Ok(Self::from_bootstrap_inputs(inputs))
    }

    pub fn from_bootstrap_inputs(inputs: CustomerRootBootstrapInputs) -> Self {
        let mut builder =
            Self::new(inputs.config, inputs.auth_package).with_customer_root(inputs.app_root);
        builder.enabled_modules = inputs.enabled_modules;
        builder
    }
}

impl<P> CustomerRootRuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    fn resolve_enabled_modules_from_customer_root(&self) -> Result<Vec<String>, RuntimeBuildError> {
        let Some(app_root) = &self.app_root else {
            return Err(RuntimeBuildError::CustomerRootNotConfigured);
        };
        let manifest_path = app_root.join("app.toml");
        let manifest = load_customer_root_manifest(&manifest_path)
            .map_err(|error| match error {
                RuntimeBootstrapError::ManifestLoad { path, reason } => {
                    RuntimeBuildError::CustomerManifestLoad { path, reason }
                }
                other => RuntimeBuildError::CustomerManifestLoad {
                    path: manifest_path.clone(),
                    reason: other.to_string(),
                },
            })?;
        validate_customer_root_manifest_alignment(&manifest, &self.config).map_err(|reason| {
            RuntimeBuildError::CustomerManifestLoad {
                path: manifest_path,
                reason,
            }
        })?;
        Ok(manifest.modules.enabled)
    }
}

impl CustomerRootBootstrapInputs {
    pub fn from_env() -> Result<Self, RuntimeBootstrapError> {
        let app_root = env::current_dir().map_err(RuntimeBootstrapError::CurrentDirectory)?;
        let config_path = discover_default_config_path(&app_root).ok_or_else(|| {
            RuntimeBootstrapError::ConfigNotFound {
                app_root: app_root.clone(),
            }
        })?;
        Self::from_paths(app_root, config_path)
    }

    pub fn from_paths(
        app_root: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
    ) -> Result<Self, RuntimeBootstrapError> {
        let app_root = app_root.as_ref().to_path_buf();
        let config_path = resolve_path(&app_root, config_path.as_ref());
        let manifest_path = app_root.join("app.toml");
        let config = PlatformConfig::from_file(&config_path).map_err(|error| {
            RuntimeBootstrapError::ConfigLoad {
                path: config_path.clone(),
                reason: error.to_string(),
            }
        })?;
        let manifest = load_customer_root_manifest(&manifest_path)?;
        validate_customer_root_manifest_alignment(&manifest, &config).map_err(|reason| {
            RuntimeBootstrapError::ManifestLoad {
                path: manifest_path.clone(),
                reason,
            }
        })?;
        let auth_package_name = config.auth.package.clone();
        let auth_package =
            load_auth_model_package_at(&auth_package_name, &app_root).map_err(|error| {
                RuntimeBootstrapError::AuthPackageLoad {
                    package: auth_package_name.clone(),
                    app_root: app_root.clone(),
                    reason: error.to_string(),
                }
            })?;

        Ok(Self {
            app_root,
            manifest_path,
            enabled_modules: manifest.modules.enabled,
            config_path,
            config,
            auth_package_name,
            auth_package,
        })
    }
}

fn resolve_path(app_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        app_root.join(path)
    }
}

fn discover_default_config_path(app_root: &Path) -> Option<PathBuf> {
    env::var("DAVENDA_CONFIG")
        .ok()
        .map(PathBuf::from)
        .map(|path| resolve_path(app_root, &path))
        .filter(|path| path.is_file())
        .or_else(|| {
            [
                PathBuf::from("platform.toml"),
                PathBuf::from("platform.dev.toml"),
                PathBuf::from("config/platform.toml"),
                PathBuf::from("davenda.toml"),
                PathBuf::from("config/davenda.toml"),
            ]
            .into_iter()
            .map(|path| app_root.join(path))
            .find(|path| path.is_file())
        })
}

fn load_customer_root_manifest(
    manifest_path: &Path,
) -> Result<CustomerRootManifestDocument, RuntimeBootstrapError> {
    let source = std::fs::read_to_string(manifest_path).map_err(|error| {
        RuntimeBootstrapError::ManifestLoad {
            path: manifest_path.to_path_buf(),
            reason: error.to_string(),
        }
    })?;
    toml::from_str(&source).map_err(|error| RuntimeBootstrapError::ManifestLoad {
        path: manifest_path.to_path_buf(),
        reason: error.to_string(),
    })
}

fn validate_customer_root_manifest_alignment(
    manifest: &CustomerRootManifestDocument,
    config: &PlatformConfig,
) -> Result<(), String> {
    if manifest.app.name != config.app.name {
        return Err(format!(
            "customer app manifest app `{}` does not match runtime config app `{}`",
            manifest.app.name, config.app.name
        ));
    }
    if manifest.auth.package != config.auth.package {
        return Err(format!(
            "customer app manifest auth package `{}` does not match runtime config auth package `{}`",
            manifest.auth.package, config.auth.package
        ));
    }
    if manifest.i18n.default_locale != config.i18n.default_locale {
        return Err(format!(
            "customer app manifest default locale `{}` does not match runtime config default locale `{}`",
            manifest.i18n.default_locale, config.i18n.default_locale
        ));
    }
    if sorted_strings(manifest.i18n.supported_locales.clone())
        != sorted_strings(config.i18n.supported_locales.clone())
    {
        return Err(format!(
            "customer app manifest supported locales {:?} do not match runtime config supported locales {:?}",
            sorted_strings(manifest.i18n.supported_locales.clone()),
            sorted_strings(config.i18n.supported_locales.clone())
        ));
    }
    if manifest.domains.canonical != config.seo.canonical_host {
        return Err(format!(
            "customer app manifest canonical host `{}` does not match runtime config canonical host `{}`",
            manifest.domains.canonical, config.seo.canonical_host
        ));
    }
    if sorted_strings(manifest.modules.enabled.clone())
        != sorted_strings(config.modules.enabled.clone())
    {
        return Err(format!(
            "customer app manifest modules {:?} do not match runtime config modules {:?}",
            sorted_strings(manifest.modules.enabled.clone()),
            sorted_strings(config.modules.enabled.clone())
        ));
    }
    Ok(())
}

fn sorted_strings(values: Vec<String>) -> Vec<String> {
    let mut values = values;
    values.sort();
    values
}

pub(crate) fn resolve_enabled_customer_modules(
    enabled_modules: &[String],
    modules: Vec<Box<dyn PlatformModule>>,
) -> Result<Vec<Box<dyn PlatformModule>>, RuntimeBuildError> {
    let enabled = enabled_modules.iter().cloned().collect::<BTreeSet<_>>();
    let mut linked = BTreeSet::new();
    let mut selected = Vec::new();
    for module in modules {
        let name = module.manifest().name;
        linked.insert(name.clone());
        if enabled.contains(&name) {
            selected.push(module);
        }
    }
    let missing = enabled
        .into_iter()
        .filter(|module| !linked.contains(module))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(RuntimeBuildError::CustomerManifestMissingLinkedModules { modules: missing });
    }
    Ok(selected)
}
