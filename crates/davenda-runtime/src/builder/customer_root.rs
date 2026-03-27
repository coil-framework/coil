use super::*;
use davenda_auth::{LoadedAuthModelPackage, load_auth_model_package_at};
use std::env;
use std::path::Path;
use std::path::PathBuf;

/// Explicit runtime bootstrap entrypoint for ADR 96 customer-root binaries/workspaces.
///
/// This keeps the linked-customer composition path visible in code without forcing every
/// customer binary to start from the lower-level `RuntimeBuilder::new(...)` surface.
pub struct CustomerRootRuntimeBuilder<P> {
    inner: RuntimeBuilder<P>,
}

pub struct CustomerRootBootstrapInputs {
    pub app_root: PathBuf,
    pub config_path: PathBuf,
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub auth_package: LoadedAuthModelPackage,
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

pub fn customer_root_runtime_from_env(
    auth_package_name: impl AsRef<str>,
) -> Result<CustomerRootRuntimeBuilder<LoadedAuthModelPackage>, RuntimeBootstrapError> {
    CustomerRootRuntimeBuilder::from_env(auth_package_name)
}

pub fn customer_root_runtime_from_paths(
    app_root: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
    auth_package_name: impl AsRef<str>,
) -> Result<CustomerRootRuntimeBuilder<LoadedAuthModelPackage>, RuntimeBootstrapError> {
    CustomerRootRuntimeBuilder::from_paths(app_root, config_path, auth_package_name)
}

pub fn customer_root_bootstrap_inputs_from_env(
    auth_package_name: impl AsRef<str>,
) -> Result<CustomerRootBootstrapInputs, RuntimeBootstrapError> {
    CustomerRootBootstrapInputs::from_env(auth_package_name)
}

pub fn customer_root_bootstrap_inputs_from_paths(
    app_root: impl AsRef<Path>,
    config_path: impl AsRef<Path>,
    auth_package_name: impl AsRef<str>,
) -> Result<CustomerRootBootstrapInputs, RuntimeBootstrapError> {
    CustomerRootBootstrapInputs::from_paths(app_root, config_path, auth_package_name)
}

impl<P> CustomerRootRuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            inner: RuntimeBuilder::new(config, auth_package),
        }
    }

    /// Mount a customer workspace/app root that contains the checked-in template tree.
    ///
    /// This is the customer-root equivalent of `with_template_root(...)`.
    pub fn with_customer_root<A>(mut self, root: A) -> Self
    where
        A: Into<PathBuf>,
    {
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
        self.inner.build()
    }

    pub fn run_from_env(self) -> Result<(), RuntimeBootstrapError> {
        self.inner.run_from_env()
    }
}

impl CustomerRootRuntimeBuilder<LoadedAuthModelPackage> {
    pub fn from_env(auth_package_name: impl AsRef<str>) -> Result<Self, RuntimeBootstrapError> {
        let app_root = env::current_dir().map_err(RuntimeBootstrapError::CurrentDirectory)?;
        let config_path = discover_default_config_path(&app_root).ok_or_else(|| {
            RuntimeBootstrapError::ConfigNotFound {
                app_root: app_root.clone(),
            }
        })?;
        Self::from_paths(app_root, config_path, auth_package_name)
    }

    pub fn from_paths(
        app_root: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
        auth_package_name: impl AsRef<str>,
    ) -> Result<Self, RuntimeBootstrapError> {
        let inputs =
            CustomerRootBootstrapInputs::from_paths(app_root, config_path, auth_package_name)?;
        Ok(Self::from_bootstrap_inputs(inputs))
    }

    pub fn from_bootstrap_inputs(inputs: CustomerRootBootstrapInputs) -> Self {
        Self::new(inputs.config, inputs.auth_package).with_customer_root(inputs.app_root)
    }
}

impl CustomerRootBootstrapInputs {
    pub fn from_env(auth_package_name: impl AsRef<str>) -> Result<Self, RuntimeBootstrapError> {
        let app_root = env::current_dir().map_err(RuntimeBootstrapError::CurrentDirectory)?;
        let config_path = discover_default_config_path(&app_root).ok_or_else(|| {
            RuntimeBootstrapError::ConfigNotFound {
                app_root: app_root.clone(),
            }
        })?;
        Self::from_paths(app_root, config_path, auth_package_name)
    }

    pub fn from_paths(
        app_root: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
        auth_package_name: impl AsRef<str>,
    ) -> Result<Self, RuntimeBootstrapError> {
        let app_root = app_root.as_ref().to_path_buf();
        let config_path = resolve_path(&app_root, config_path.as_ref());
        let config = PlatformConfig::from_file(&config_path).map_err(|error| {
            RuntimeBootstrapError::ConfigLoad {
                path: config_path.clone(),
                reason: error.to_string(),
            }
        })?;
        let auth_package_name = auth_package_name.as_ref().to_string();
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
