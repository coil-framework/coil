#![forbid(unsafe_code)]

use coil_customer_sdk::{
    AuditFacade, AuthFacade, BackendError, CheckoutHooks, CommerceFacade, CustomerBackendPlugin,
    CustomerHookRegistry, CustomerPluginDescriptor, OrderDraft, OrderReviewDecision,
    OutboundHttpFacade, RegisteredHookKind, RequestContext, VerifiedWebhook, VerifiedWebhookHooks,
    WebhookHandlingResult,
};
use std::sync::Arc;

pub use shoppr_loyalty_backend::{
    CrmContactRoute, CrmContactUpdate, LoyaltyPreviewRequest, LoyaltyPreviewResponse,
    OrderReviewRequest, OrderReviewResponse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShopprBackend {
    inner: shoppr_loyalty_backend::ShopprCustomerBackend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopprLinkedPluginSummary {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub documentation_url: Option<String>,
    pub hook_kinds: Vec<RegisteredHookKind>,
}

pub fn plugin() -> ShopprBackend {
    ShopprBackend {
        inner: shoppr_loyalty_backend::plugin(),
    }
}

impl ShopprBackend {
    pub fn preview_loyalty(&self, request: &LoyaltyPreviewRequest) -> LoyaltyPreviewResponse {
        self.inner.preview_loyalty(request)
    }

    pub fn review_checkout_order(&self, request: &OrderReviewRequest) -> OrderReviewResponse {
        self.inner.review_checkout_order(request)
    }

    pub fn route_crm_contact_update(&self, update: &CrmContactUpdate) -> CrmContactRoute {
        self.inner.route_crm_contact_update(update)
    }
}

#[derive(Default)]
struct RecordingRegistry {
    hook_kinds: Vec<RegisteredHookKind>,
}

impl CustomerHookRegistry for RecordingRegistry {
    fn register_checkout_hooks(
        &mut self,
        _hooks: Arc<dyn CheckoutHooks>,
    ) -> Result<(), BackendError> {
        self.hook_kinds.push(RegisteredHookKind::Checkout);
        Ok(())
    }

    fn register_cms_hooks(
        &mut self,
        _hooks: Arc<dyn coil_customer_sdk::CmsHooks>,
    ) -> Result<(), BackendError> {
        self.hook_kinds.push(RegisteredHookKind::CmsPagePublish);
        Ok(())
    }

    fn register_render_model_hooks(
        &mut self,
        _hooks: Arc<dyn coil_customer_sdk::RenderModelHooks>,
    ) -> Result<(), BackendError> {
        self.hook_kinds.push(RegisteredHookKind::RenderModel);
        Ok(())
    }

    fn register_verified_webhook_hooks(
        &mut self,
        _hooks: Arc<dyn VerifiedWebhookHooks>,
    ) -> Result<(), BackendError> {
        self.hook_kinds.push(RegisteredHookKind::VerifiedWebhook);
        Ok(())
    }

    fn register_verified_webhook_asset_hooks(
        &mut self,
        _hooks: Arc<dyn coil_customer_sdk::VerifiedWebhookAssetHooks>,
    ) -> Result<(), BackendError> {
        self.hook_kinds
            .push(RegisteredHookKind::VerifiedWebhookAssets);
        Ok(())
    }
}

pub fn linked_plugin_summary() -> ShopprLinkedPluginSummary {
    let plugin = plugin();
    let descriptor = plugin.descriptor();
    let mut registry = RecordingRegistry::default();
    plugin
        .register(&mut registry)
        .expect("Shoppr linked backend registration should succeed");
    ShopprLinkedPluginSummary {
        id: descriptor.id,
        display_name: descriptor.display_name,
        version: descriptor.version,
        documentation_url: descriptor.documentation_url,
        hook_kinds: registry.hook_kinds,
    }
}

impl CustomerBackendPlugin for ShopprBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "shoppr-backend",
            "Shoppr Linked Backend",
            env!("CARGO_PKG_VERSION"),
        )
        .with_documentation_url("apps/shoppr/backend/README.md")
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        let hooks = Arc::new(*self);
        registry.register_checkout_hooks(hooks.clone())?;
        registry.register_verified_webhook_hooks(hooks)?;
        Ok(())
    }
}

impl CheckoutHooks for ShopprBackend {
    fn review_order(
        &self,
        ctx: &RequestContext,
        order: &OrderDraft,
        commerce: &dyn CommerceFacade,
        auth: &dyn AuthFacade,
        audit: &dyn AuditFacade,
    ) -> Result<OrderReviewDecision, BackendError> {
        self.inner.review_order(ctx, order, commerce, auth, audit)
    }
}

impl VerifiedWebhookHooks for ShopprBackend {
    fn handle_verified_webhook(
        &self,
        ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        http: &dyn OutboundHttpFacade,
        jobs: &dyn coil_customer_sdk::JobsFacade,
        repositories: &dyn coil_customer_sdk::RepositoryFacade,
        audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        self.inner
            .handle_verified_webhook(ctx, webhook, http, jobs, repositories, audit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_customer_backend_descriptor_is_stable() {
        let descriptor = coil_customer_sdk::CustomerBackendPlugin::descriptor(&plugin());

        assert_eq!(descriptor.id, "shoppr-backend");
        assert_eq!(descriptor.display_name, "Shoppr Linked Backend");
        assert_eq!(descriptor.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn linked_customer_backend_summary_reports_registered_hook_kinds() {
        let summary = linked_plugin_summary();

        assert_eq!(summary.id, "shoppr-backend");
        assert_eq!(summary.display_name, "Shoppr Linked Backend");
        assert_eq!(
            summary.hook_kinds,
            vec![
                RegisteredHookKind::Checkout,
                RegisteredHookKind::VerifiedWebhook,
            ]
        );
        assert_eq!(
            summary.documentation_url.as_deref(),
            Some("apps/shoppr/backend/README.md")
        );
    }
}
