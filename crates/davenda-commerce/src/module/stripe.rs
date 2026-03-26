use davenda_core::{
    CoreServiceDependency, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, ServiceRegistry,
};

pub struct CommercePaymentsStripeModule;

impl CommercePaymentsStripeModule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommercePaymentsStripeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for CommercePaymentsStripeModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new("commerce-payments-stripe")
            .with_config_namespace("commerce_payments_stripe".to_string())
            .with_module_dependencies(vec![ModuleDependency::required(
                "commerce",
                "Stripe payment confirmation extends the base commerce checkout and webhook lifecycle",
            )])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Observability,
            ])
            .with_behaviors(vec![ModuleBehavior::AsyncJobs])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            "commerce-payments-stripe".to_string(),
            "module.commerce.payments.stripe",
            "Stripe-backed payment confirmation and webhook reconciliation for commerce checkout",
        )
    }
}
