use super::*;
use std::path::PathBuf;

/// Explicit runtime bootstrap entrypoint for ADR 96 customer-root binaries/workspaces.
///
/// This keeps the linked-customer composition path visible in code without forcing every
/// customer binary to start from the lower-level `RuntimeBuilder::new(...)` surface.
pub struct CustomerRootRuntimeBuilder<P> {
    inner: RuntimeBuilder<P>,
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

    pub fn with_boxed_module(mut self, module: Box<dyn PlatformModule>) -> Self {
        self.inner = self.inner.with_boxed_module(module);
        self
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
}
