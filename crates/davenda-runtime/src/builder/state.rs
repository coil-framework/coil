use super::*;
use davenda_template::TemplateDefinition;
use crate::builder::assembly;

pub struct RuntimeBuilder<P> {
    config: PlatformConfig,
    auth_package: P,
    modules: Vec<Box<dyn PlatformModule>>,
    extensions: Vec<InstalledExtension>,
    templates: Vec<TemplateDefinition>,
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
    pub(crate) extensions: Vec<InstalledExtension>,
    pub(crate) templates: Vec<TemplateDefinition>,
    pub(crate) storage_policies: StoragePolicySet,
    pub(crate) routes: Vec<RouteDefinition>,
    pub(crate) handlers: Vec<HandlerDefinition>,
    pub(crate) feature_flags: Vec<FeatureFlag>,
    pub(crate) maintenance_mode: Option<MaintenanceMode>,
}

impl<P> RuntimeBuilder<P>
where
    P: AuthModelPackage,
{
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            config,
            auth_package,
            modules: Vec::new(),
            extensions: Vec::new(),
            templates: Vec::new(),
            storage_policies: StoragePolicySet::default(),
            routes: Vec::new(),
            handlers: Vec::new(),
            feature_flags: Vec::new(),
            maintenance_mode: None,
        }
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn with_boxed_module(mut self, module: Box<dyn PlatformModule>) -> Self {
        self.modules.push(module);
        self
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

    pub fn build(self) -> Result<RuntimePlan, RuntimeBuildError> {
        assembly::build_runtime_plan(self)
    }

    pub(crate) fn into_parts(self) -> RuntimeBuilderParts<P> {
        RuntimeBuilderParts {
            config: self.config,
            auth_package: self.auth_package,
            modules: self.modules,
            extensions: self.extensions,
            templates: self.templates,
            storage_policies: self.storage_policies,
            routes: self.routes,
            handlers: self.handlers,
            feature_flags: self.feature_flags,
            maintenance_mode: self.maintenance_mode,
        }
    }
}
