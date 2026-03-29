use std::sync::Arc;

use crate::{
    BackendError, CheckoutHooks, CmsHooks, CustomerPluginDescriptor, RenderModelHooks,
    VerifiedWebhookAssetHooks, VerifiedWebhookHooks,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisteredHookKind {
    Checkout,
    CmsPagePublish,
    RenderModel,
    VerifiedWebhook,
    VerifiedWebhookAssets,
}

impl RegisteredHookKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Checkout => "checkout",
            Self::CmsPagePublish => "cms.page_publish",
            Self::RenderModel => "render_model",
            Self::VerifiedWebhook => "verified_webhook",
            Self::VerifiedWebhookAssets => "verified_webhook.assets",
        }
    }
}

pub trait CustomerHookRegistry {
    fn register_checkout_hooks(
        &mut self,
        hooks: Arc<dyn CheckoutHooks>,
    ) -> Result<(), BackendError>;

    fn register_cms_hooks(&mut self, hooks: Arc<dyn CmsHooks>) -> Result<(), BackendError>;

    fn register_render_model_hooks(
        &mut self,
        hooks: Arc<dyn RenderModelHooks>,
    ) -> Result<(), BackendError>;

    fn register_verified_webhook_hooks(
        &mut self,
        hooks: Arc<dyn VerifiedWebhookHooks>,
    ) -> Result<(), BackendError>;

    fn register_verified_webhook_asset_hooks(
        &mut self,
        hooks: Arc<dyn VerifiedWebhookAssetHooks>,
    ) -> Result<(), BackendError>;
}

pub trait CustomerBackendPlugin: Send + Sync + 'static {
    fn descriptor(&self) -> CustomerPluginDescriptor;

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError>;
}
