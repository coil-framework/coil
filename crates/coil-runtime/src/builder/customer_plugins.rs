use coil_customer_sdk::{
    BackendError, CheckoutHooks, CmsHooks, VerifiedWebhookAssetHooks, VerifiedWebhookHooks,
};
use std::fmt;
use std::sync::Arc;

pub use coil_customer_sdk::{CustomerBackendPlugin, CustomerHookRegistry, RegisteredHookKind};

#[derive(Clone, Default)]
pub(crate) struct CustomerHookSet {
    pub(crate) checkout: Vec<Arc<dyn CheckoutHooks>>,
    pub(crate) cms: Vec<Arc<dyn CmsHooks>>,
    pub(crate) verified_webhooks: Vec<Arc<dyn VerifiedWebhookHooks>>,
    pub(crate) verified_webhook_assets: Vec<Arc<dyn VerifiedWebhookAssetHooks>>,
}

impl fmt::Debug for CustomerHookSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CustomerHookSet")
            .field("checkout", &self.checkout.len())
            .field("cms", &self.cms.len())
            .field("verified_webhooks", &self.verified_webhooks.len())
            .field(
                "verified_webhook_assets",
                &self.verified_webhook_assets.len(),
            )
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCustomerPluginSummary {
    pub plugin_id: String,
    pub display_name: String,
    pub version: String,
    pub registered_hooks: Vec<RegisteredHookKind>,
}

#[derive(Default)]
pub(crate) struct RuntimeCustomerHookRegistry {
    hooks: CustomerHookSet,
    registered_hooks: Vec<RegisteredHookKind>,
}

impl RuntimeCustomerHookRegistry {
    pub(crate) fn into_parts(self) -> (CustomerHookSet, Vec<RegisteredHookKind>) {
        (self.hooks, self.registered_hooks)
    }
}

impl CustomerHookRegistry for RuntimeCustomerHookRegistry {
    fn register_checkout_hooks(
        &mut self,
        hooks: Arc<dyn CheckoutHooks>,
    ) -> Result<(), BackendError> {
        self.hooks.checkout.push(hooks);
        self.registered_hooks.push(RegisteredHookKind::Checkout);
        Ok(())
    }

    fn register_cms_hooks(&mut self, hooks: Arc<dyn CmsHooks>) -> Result<(), BackendError> {
        self.hooks.cms.push(hooks);
        self.registered_hooks
            .push(RegisteredHookKind::CmsPagePublish);
        Ok(())
    }

    fn register_verified_webhook_hooks(
        &mut self,
        hooks: Arc<dyn VerifiedWebhookHooks>,
    ) -> Result<(), BackendError> {
        self.hooks.verified_webhooks.push(hooks);
        self.registered_hooks
            .push(RegisteredHookKind::VerifiedWebhook);
        Ok(())
    }

    fn register_verified_webhook_asset_hooks(
        &mut self,
        hooks: Arc<dyn VerifiedWebhookAssetHooks>,
    ) -> Result<(), BackendError> {
        self.hooks.verified_webhook_assets.push(hooks);
        self.registered_hooks
            .push(RegisteredHookKind::VerifiedWebhookAssets);
        Ok(())
    }
}
