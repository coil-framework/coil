use super::*;
use crate::builder::assembly;
use davenda_i18n::TranslationCatalog;
use davenda_template::TemplateDefinition;
use std::env;
use std::path::Path;
use std::path::PathBuf;

#[derive(Default)]
pub struct Builder {
    modules: Vec<Box<dyn PlatformModule>>,
    customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn register_customer_plugin<C>(mut self, plugin: C) -> Self
    where
        C: CustomerBackendPlugin,
    {
        self.customer_plugins.push(Box::new(plugin));
        self
    }

    pub fn build_from_paths(
        self,
        app_root: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
    ) -> Result<RuntimePlan, RuntimeBootstrapError> {
        let bootstrap = CustomerRootBootstrapInputs::from_paths(app_root, config_path)?;
        self.build_from_bootstrap_inputs(bootstrap)
    }

    pub fn run_from_env(self) -> Result<(), RuntimeBootstrapError> {
        let bind_override = env::var("DAVENDA_BIND").ok();
        self.build_from_bootstrap_inputs(CustomerRootBootstrapInputs::from_env()?)?
            .serve_from_env(bind_override)
    }

    fn build_from_bootstrap_inputs(
        self,
        bootstrap: CustomerRootBootstrapInputs,
    ) -> Result<RuntimePlan, RuntimeBootstrapError> {
        let mut builder = RuntimeBuilder::new(bootstrap.config, bootstrap.auth_package)
            .with_template_root(bootstrap.app_root)
            .with_translation_catalogs(bootstrap.translation_catalogs);
        for module in self.modules {
            builder = builder.with_boxed_module(module);
        }
        for plugin in self.customer_plugins {
            builder = builder.with_boxed_customer_plugin(plugin);
        }
        builder = builder
            .resolve_enabled_customer_modules(&bootstrap.enabled_modules)
            .map_err(RuntimeBootstrapError::Build)?;
        builder.build().map_err(RuntimeBootstrapError::Build)
    }
}

pub struct RuntimeBuilder<P> {
    config: PlatformConfig,
    auth_package: P,
    modules: Vec<Box<dyn PlatformModule>>,
    customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
    translation_catalogs: Vec<TranslationCatalog>,
    extensions: Vec<InstalledExtension>,
    templates: Vec<TemplateDefinition>,
    template_roots: Vec<PathBuf>,
    storage_policies: StoragePolicySet,
    routes: Vec<RouteDefinition>,
    handlers: Vec<HandlerDefinition>,
    feature_flags: Vec<FeatureFlag>,
    maintenance_mode: Option<MaintenanceMode>,
}

pub(crate) struct RuntimeBuilderParts<P> {
    pub(crate) config: PlatformConfig,
    pub(crate) auth_package: P,
    pub(crate) modules: Vec<Box<dyn PlatformModule>>,
    pub(crate) customer_plugins: Vec<Box<dyn CustomerBackendPlugin>>,
    pub(crate) translation_catalogs: Vec<TranslationCatalog>,
    pub(crate) extensions: Vec<InstalledExtension>,
    pub(crate) templates: Vec<TemplateDefinition>,
    pub(crate) template_roots: Vec<PathBuf>,
    pub(crate) storage_policies: StoragePolicySet,
    pub(crate) routes: Vec<RouteDefinition>,
    pub(crate) handlers: Vec<HandlerDefinition>,
    pub(crate) feature_flags: Vec<FeatureFlag>,
    pub(crate) maintenance_mode: Option<MaintenanceMode>,
}

impl<P> RuntimeBuilder<P>
where
    P: AuthModelPackage + 'static,
{
    /// Lower-level runtime builder for direct platform/runtime composition.
    ///
    /// Customer-root binaries should prefer `RuntimeBuilder::for_customer_root(...)` so the
    /// linked-customer composition path stays explicit in code.
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            config,
            auth_package,
            modules: Vec::new(),
            customer_plugins: Vec::new(),
            translation_catalogs: Vec::new(),
            extensions: Vec::new(),
            templates: Vec::new(),
            template_roots: Vec::new(),
            storage_policies: StoragePolicySet::default(),
            routes: Vec::new(),
            handlers: Vec::new(),
            feature_flags: Vec::new(),
            maintenance_mode: None,
        }
    }

    /// Explicit ADR 96 entrypoint for linked customer-root binaries/workspaces.
    pub fn for_customer_root(
        config: PlatformConfig,
        auth_package: P,
    ) -> CustomerRootRuntimeBuilder<P> {
        CustomerRootRuntimeBuilder::new(config, auth_package)
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn register_module<M>(self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.with_module(module)
    }

    pub fn with_boxed_module(mut self, module: Box<dyn PlatformModule>) -> Self {
        self.modules.push(module);
        self
    }

    pub fn register_customer_plugin<C>(mut self, plugin: C) -> Self
    where
        C: CustomerBackendPlugin,
    {
        self.customer_plugins.push(Box::new(plugin));
        self
    }

    pub fn with_boxed_customer_plugin(mut self, plugin: Box<dyn CustomerBackendPlugin>) -> Self {
        self.customer_plugins.push(plugin);
        self
    }

    pub fn with_translation_catalog(mut self, catalog: TranslationCatalog) -> Self {
        self.translation_catalogs.push(catalog);
        self
    }

    pub fn with_translation_catalogs<I>(mut self, catalogs: I) -> Self
    where
        I: IntoIterator<Item = TranslationCatalog>,
    {
        self.translation_catalogs.extend(catalogs);
        self
    }

    pub fn with_customer_plugin<C>(self, plugin: C) -> Self
    where
        C: CustomerBackendPlugin,
    {
        self.register_customer_plugin(plugin)
    }

    pub fn with_installed_extension(mut self, extension: InstalledExtension) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn with_template(mut self, template: TemplateDefinition) -> Self {
        self.templates.push(template);
        self
    }

    pub fn with_templates<I>(mut self, templates: I) -> Self
    where
        I: IntoIterator<Item = TemplateDefinition>,
    {
        self.templates.extend(templates);
        self
    }

    pub fn with_template_root<A>(mut self, root: A) -> Self
    where
        A: Into<PathBuf>,
    {
        self.template_roots.push(root.into());
        self
    }

    pub fn with_storage_policy_rule(mut self, rule: PathPolicyRule) -> Self {
        self.storage_policies = self.storage_policies.with_rule(rule);
        self
    }

    pub fn with_storage_policies(mut self, policies: StoragePolicySet) -> Self {
        self.storage_policies = policies;
        self
    }

    pub fn with_route(mut self, route: RouteDefinition) -> Self {
        self.routes.push(route);
        self
    }

    pub fn with_handler(mut self, handler: HandlerDefinition) -> Self {
        self.handlers.push(handler);
        self
    }

    pub fn with_feature_flag(mut self, feature_flag: FeatureFlag) -> Self {
        self.feature_flags.push(feature_flag);
        self
    }

    pub fn with_maintenance_mode(mut self, maintenance_mode: MaintenanceMode) -> Self {
        self.maintenance_mode = Some(maintenance_mode);
        self
    }

    pub(crate) fn resolve_enabled_customer_modules(
        mut self,
        enabled_modules: &[String],
    ) -> Result<Self, RuntimeBuildError> {
        self.modules = crate::builder::customer_root::resolve_enabled_customer_modules(
            enabled_modules,
            self.modules,
        )?;
        Ok(self)
    }

    pub fn build(self) -> Result<RuntimePlan, RuntimeBuildError> {
        assembly::build_runtime_plan(self)
    }

    pub fn run_from_env(self) -> Result<(), RuntimeBootstrapError> {
        self.build()?.serve_from_env(env::var("DAVENDA_BIND").ok())
    }

    pub(crate) fn into_parts(self) -> RuntimeBuilderParts<P> {
        RuntimeBuilderParts {
            config: self.config,
            auth_package: self.auth_package,
            modules: self.modules,
            customer_plugins: self.customer_plugins,
            translation_catalogs: self.translation_catalogs,
            extensions: self.extensions,
            templates: self.templates,
            template_roots: self.template_roots,
            storage_policies: self.storage_policies,
            routes: self.routes,
            handlers: self.handlers,
            feature_flags: self.feature_flags,
            maintenance_mode: self.maintenance_mode,
        }
    }
}

impl RuntimeBuilder<davenda_auth::LoadedAuthModelPackage> {
    pub fn for_customer_root_from_env() -> Result<
        CustomerRootRuntimeBuilder<davenda_auth::LoadedAuthModelPackage>,
        RuntimeBootstrapError,
    > {
        CustomerRootRuntimeBuilder::from_env()
    }

    pub fn for_customer_root_from_paths(
        app_root: impl AsRef<std::path::Path>,
        config_path: impl AsRef<std::path::Path>,
    ) -> Result<
        CustomerRootRuntimeBuilder<davenda_auth::LoadedAuthModelPackage>,
        RuntimeBootstrapError,
    > {
        CustomerRootRuntimeBuilder::from_paths(app_root, config_path)
    }
}
