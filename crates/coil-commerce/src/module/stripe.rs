use crate::CommerceModelError;
use coil_config::{PlatformConfig, SecretRef};
use coil_core::{
    CoreServiceDependency, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, ServiceRegistry,
};

pub const STRIPE_MODULE_NAME: &str = "commerce-payments-stripe";
pub const STRIPE_CONFIG_NAMESPACE: &str = "commerce_payments_stripe";
pub const STRIPE_PROVIDER_CODE: &str = "stripe";
pub const STRIPE_PROVIDER_LABEL: &str = "Stripe";
pub const STRIPE_SERVICE_ID: &str = "module.commerce.payments.stripe";
pub const STRIPE_PAYMENT_WEBHOOK_ROUTE: &str = "/webhooks/commerce/payment-provider";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripeCheckoutMode {
    WebhookConfirmation,
    HostedCheckout,
}

impl StripeCheckoutMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WebhookConfirmation => "webhook-confirmation",
            Self::HostedCheckout => "hosted-checkout",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripeProviderMetadata {
    pub module_name: String,
    pub service_id: String,
    pub provider_code: String,
    pub provider_label: String,
    pub checkout_mode: StripeCheckoutMode,
    pub webhook_route: String,
    pub publishable_key_ref: String,
    pub webhook_secret_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommercePaymentsStripeConfig {
    pub provider: String,
    pub checkout_mode: StripeCheckoutMode,
    pub publishable_key: SecretRef,
    pub webhook_secret: SecretRef,
}

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

impl CommercePaymentsStripeModule {
    pub fn provider_metadata(config: &CommercePaymentsStripeConfig) -> StripeProviderMetadata {
        StripeProviderMetadata {
            module_name: STRIPE_MODULE_NAME.to_string(),
            service_id: STRIPE_SERVICE_ID.to_string(),
            provider_code: STRIPE_PROVIDER_CODE.to_string(),
            provider_label: STRIPE_PROVIDER_LABEL.to_string(),
            checkout_mode: config.checkout_mode,
            webhook_route: STRIPE_PAYMENT_WEBHOOK_ROUTE.to_string(),
            publishable_key_ref: config.publishable_key.redacted(),
            webhook_secret_ref: config.webhook_secret.redacted(),
        }
    }
}

impl CommercePaymentsStripeConfig {
    pub fn from_platform_config(
        config: &PlatformConfig,
    ) -> Result<Option<Self>, CommerceModelError> {
        let enabled = config
            .modules
            .enabled
            .iter()
            .any(|module| module == STRIPE_MODULE_NAME);
        let Some(settings) = config.modules.settings.get(STRIPE_MODULE_NAME) else {
            return if enabled {
                Err(CommerceModelError::MissingModuleSetting {
                    module: STRIPE_MODULE_NAME.to_string(),
                    field: format!("[modules.\"{STRIPE_MODULE_NAME}\"]"),
                })
            } else {
                Ok(None)
            };
        };
        let Some(table) = settings.as_table() else {
            return Err(CommerceModelError::InvalidModuleSetting {
                module: STRIPE_MODULE_NAME.to_string(),
                field: format!("[modules.\"{STRIPE_MODULE_NAME}\"]"),
                reason: "expected a table of provider settings".to_string(),
            });
        };

        let provider = module_string_setting(table, "provider")?.to_ascii_lowercase();
        if provider != STRIPE_PROVIDER_CODE {
            return Err(CommerceModelError::UnsupportedModuleSetting {
                module: STRIPE_MODULE_NAME.to_string(),
                field: "provider".to_string(),
                value: provider,
            });
        }

        let checkout_mode_raw = module_string_setting(table, "checkout_mode")?.to_ascii_lowercase();
        let checkout_mode = match checkout_mode_raw.as_str() {
            "webhook-confirmation" => StripeCheckoutMode::WebhookConfirmation,
            "hosted-checkout" | "hosted_checkout" | "stripe-hosted-checkout" => {
                StripeCheckoutMode::HostedCheckout
            }
            other => {
                return Err(CommerceModelError::UnsupportedModuleSetting {
                    module: STRIPE_MODULE_NAME.to_string(),
                    field: "checkout_mode".to_string(),
                    value: other.to_string(),
                });
            }
        };

        let publishable_key = module_secret_setting(table, "publishable_key")?;
        let webhook_secret = module_secret_setting(table, "webhook_secret")?;

        Ok(Some(Self {
            provider,
            checkout_mode,
            publishable_key,
            webhook_secret,
        }))
    }

    pub fn provider_metadata(&self) -> StripeProviderMetadata {
        CommercePaymentsStripeModule::provider_metadata(self)
    }
}

impl PlatformModule for CommercePaymentsStripeModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(STRIPE_MODULE_NAME)
            .with_config_namespace(STRIPE_CONFIG_NAMESPACE.to_string())
            .with_module_dependencies(vec![ModuleDependency::required(
                "commerce",
                "Stripe checkout handoff and signed webhook reconciliation extend the base commerce checkout lifecycle",
            )])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Observability,
            ])
            .with_behaviors(vec![ModuleBehavior::AsyncJobs])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            STRIPE_MODULE_NAME.to_string(),
            STRIPE_SERVICE_ID,
            "Stripe checkout handoff contract, publishable-key configuration, and signed webhook reconciliation for commerce checkout",
        )
    }
}

fn module_string_setting(table: &toml::Table, field: &str) -> Result<String, CommerceModelError> {
    let value = table
        .get(field)
        .ok_or_else(|| CommerceModelError::MissingModuleSetting {
            module: STRIPE_MODULE_NAME.to_string(),
            field: field.to_string(),
        })?;
    let value = value
        .as_str()
        .ok_or_else(|| CommerceModelError::InvalidModuleSetting {
            module: STRIPE_MODULE_NAME.to_string(),
            field: field.to_string(),
            reason: "expected a non-empty string".to_string(),
        })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(CommerceModelError::InvalidModuleSetting {
            module: STRIPE_MODULE_NAME.to_string(),
            field: field.to_string(),
            reason: "expected a non-empty string".to_string(),
        });
    }
    Ok(value.to_string())
}

fn module_secret_setting(
    table: &toml::Table,
    field: &str,
) -> Result<SecretRef, CommerceModelError> {
    let value = table
        .get(field)
        .ok_or_else(|| CommerceModelError::MissingModuleSetting {
            module: STRIPE_MODULE_NAME.to_string(),
            field: field.to_string(),
        })?;
    value.clone().try_into().map_err(|error: toml::de::Error| {
        CommerceModelError::InvalidModuleSetting {
            module: STRIPE_MODULE_NAME.to_string(),
            field: field.to_string(),
            reason: error.to_string(),
        }
    })
}
