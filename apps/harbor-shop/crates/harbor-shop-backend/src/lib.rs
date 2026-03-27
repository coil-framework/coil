#![forbid(unsafe_code)]

use davenda_customer_sdk::{
    AuditFacade, AuthFacade, BackendError, CheckoutHooks, CommerceFacade, CustomerBackendPlugin,
    CustomerHookRegistry, CustomerPluginDescriptor, OrderDraft, OrderReviewDecision,
    OutboundHttpFacade, RequestContext, VerifiedWebhook, VerifiedWebhookHooks,
    WebhookHandlingResult,
};
use std::sync::Arc;

pub use harbor_loyalty_backend::{
    CrmContactRoute, CrmContactUpdate, LoyaltyPreviewRequest, LoyaltyPreviewResponse,
    OrderReviewRequest, OrderReviewResponse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HarborShopBackend {
    inner: harbor_loyalty_backend::HarborCustomerBackend,
}

pub fn plugin() -> HarborShopBackend {
    HarborShopBackend {
        inner: harbor_loyalty_backend::plugin(),
    }
}

impl HarborShopBackend {
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

impl CustomerBackendPlugin for HarborShopBackend {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-backend",
            "Harbor Shop Linked Backend",
            env!("CARGO_PKG_VERSION"),
        )
        .with_documentation_url("apps/harbor-shop/backend/README.md")
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        let hooks = Arc::new(*self);
        registry.register_checkout_hooks(hooks.clone())?;
        registry.register_verified_webhook_hooks(hooks)?;
        Ok(())
    }
}

impl CheckoutHooks for HarborShopBackend {
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

impl VerifiedWebhookHooks for HarborShopBackend {
    fn handle_verified_webhook(
        &self,
        ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        http: &dyn OutboundHttpFacade,
        jobs: &dyn davenda_customer_sdk::JobsFacade,
        audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        self.inner
            .handle_verified_webhook(ctx, webhook, http, jobs, audit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_customer_backend_descriptor_is_stable() {
        let descriptor = davenda_customer_sdk::CustomerBackendPlugin::descriptor(&plugin());

        assert_eq!(descriptor.id, "harbor-shop-backend");
        assert_eq!(descriptor.display_name, "Harbor Shop Linked Backend");
        assert_eq!(descriptor.version, env!("CARGO_PKG_VERSION"));
    }
}
