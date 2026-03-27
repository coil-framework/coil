#![forbid(unsafe_code)]

mod error;
mod facade;
mod hooks;
mod registry;
mod types;

pub use error::*;
pub use facade::*;
pub use hooks::*;
pub use registry::*;
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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

        fn register_cms_hooks(&mut self, _hooks: Arc<dyn CmsHooks>) -> Result<(), BackendError> {
            self.hook_kinds.push(RegisteredHookKind::CmsPagePublish);
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
            _hooks: Arc<dyn VerifiedWebhookAssetHooks>,
        ) -> Result<(), BackendError> {
            self.hook_kinds
                .push(RegisteredHookKind::VerifiedWebhookAssets);
            Ok(())
        }
    }

    struct ExamplePlugin;

    impl CustomerBackendPlugin for ExamplePlugin {
        fn descriptor(&self) -> CustomerPluginDescriptor {
            CustomerPluginDescriptor::new("harbor-shop-backend", "Harbor Shop Backend", "0.1.0")
        }

        fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
            let hooks = Arc::new(ExampleHooks);
            registry.register_checkout_hooks(hooks.clone())?;
            registry.register_cms_hooks(hooks.clone())?;
            registry.register_verified_webhook_hooks(hooks)?;
            Ok(())
        }
    }

    struct ExampleHooks;

    impl CheckoutHooks for ExampleHooks {
        fn review_order(
            &self,
            _ctx: &RequestContext,
            _order: &OrderDraft,
            _commerce: &dyn CommerceFacade,
            _auth: &dyn AuthFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<OrderReviewDecision, BackendError> {
            Ok(OrderReviewDecision::approved())
        }
    }

    impl CmsHooks for ExampleHooks {
        fn validate_page_publish(
            &self,
            _ctx: &RequestContext,
            _draft: &CmsPageDraft,
            _repositories: &dyn RepositoryFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<CmsPublishDecision, BackendError> {
            Ok(CmsPublishDecision::Allow)
        }
    }

    impl VerifiedWebhookHooks for ExampleHooks {
        fn handle_verified_webhook(
            &self,
            _ctx: &RequestContext,
            _webhook: &VerifiedWebhook,
            _http: &dyn OutboundHttpFacade,
            _jobs: &dyn JobsFacade,
            _repositories: &dyn RepositoryFacade,
            _audit: &dyn AuditFacade,
        ) -> Result<WebhookHandlingResult, BackendError> {
            Ok(WebhookHandlingResult::accepted(None))
        }
    }

    impl VerifiedWebhookAssetHooks for ExampleHooks {
        fn handle_verified_webhook(
            &self,
            _ctx: &RequestContext,
            _webhook: &VerifiedWebhook,
            _http: &dyn OutboundHttpFacade,
            _jobs: &dyn JobsFacade,
            _repositories: &dyn RepositoryFacade,
            _audit: &dyn AuditFacade,
            _assets: &dyn AssetsFacade,
        ) -> Result<WebhookHandlingResult, BackendError> {
            Ok(WebhookHandlingResult::accepted(None))
        }
    }

    #[test]
    fn customer_plugin_registers_explicit_hook_kinds() {
        let plugin = ExamplePlugin;
        let mut registry = RecordingRegistry::default();

        plugin.register(&mut registry).unwrap();

        assert_eq!(
            registry.hook_kinds,
            vec![
                RegisteredHookKind::Checkout,
                RegisteredHookKind::CmsPagePublish,
                RegisteredHookKind::VerifiedWebhook,
            ]
        );
    }

    #[test]
    fn order_review_decision_helpers_build_stable_variants() {
        assert_eq!(
            OrderReviewDecision::approved(),
            OrderReviewDecision::Approved
        );
        assert_eq!(
            OrderReviewDecision::rejected("checkout.policy", "blocked"),
            OrderReviewDecision::Rejected(OrderRejection::new("checkout.policy", "blocked"))
        );
    }

    #[test]
    fn registry_can_record_asset_capable_verified_webhook_hooks() {
        let hooks = Arc::new(ExampleHooks);
        let mut registry = RecordingRegistry::default();

        registry
            .register_verified_webhook_asset_hooks(hooks)
            .unwrap();

        assert_eq!(
            registry.hook_kinds,
            vec![RegisteredHookKind::VerifiedWebhookAssets]
        );
    }
}
