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
    use std::collections::BTreeMap;
    use std::sync::Arc;

    #[derive(Default)]
    struct RecordingRegistry {
        hook_kinds: Vec<RegisteredHookKind>,
    }

    struct RecordingRepository {
        read_results: Vec<RepositoryRecordSet>,
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

    impl RepositoryFacade for RecordingRepository {
        fn read(&self, query: &RepositoryQuery) -> Result<RepositoryRecordSet, BackendError> {
            self.read_results
                .iter()
                .find(|result| result.repository == query.repository)
                .cloned()
                .ok_or_else(|| {
                    BackendError::new(
                        BackendErrorKind::Unsupported,
                        "repository.test.unconfigured",
                        "test repository did not provide a matching read result",
                    )
                })
        }

        fn write(&self, _change: RepositoryWrite) -> Result<RepositoryWriteReceipt, BackendError> {
            unreachable!("recording repository write is not used in these tests")
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

    #[test]
    fn repository_facade_ext_parses_typed_catalog_and_order_records() {
        let repository = RecordingRepository {
            read_results: vec![
                RepositoryRecordSet {
                    repository: CommerceCatalogProductRecord::REPOSITORY.to_string(),
                    records: vec![RepositoryRecord {
                        id: "gold-membership".to_string(),
                        fields: BTreeMap::from([
                            ("handle".to_string(), "gold-membership".to_string()),
                            ("sku".to_string(), "membership-gold".to_string()),
                            ("title".to_string(), "Gold Membership".to_string()),
                            ("summary".to_string(), "Recurring access".to_string()),
                            ("price_minor".to_string(), "12900".to_string()),
                            ("currency".to_string(), "GBP".to_string()),
                            ("collection_handle".to_string(), "memberships".to_string()),
                            ("is_visible".to_string(), "true".to_string()),
                            ("product_kind".to_string(), "membership".to_string()),
                            ("entitlement_key".to_string(), "membership.gold".to_string()),
                        ]),
                    }],
                },
                RepositoryRecordSet {
                    repository: CommerceCatalogCollectionRecord::REPOSITORY.to_string(),
                    records: vec![RepositoryRecord {
                        id: "memberships".to_string(),
                        fields: BTreeMap::from([
                            ("handle".to_string(), "memberships".to_string()),
                            ("title".to_string(), "Memberships".to_string()),
                            ("label".to_string(), "Recurring value".to_string()),
                            (
                                "summary".to_string(),
                                "Benefits and premium access".to_string(),
                            ),
                            ("is_visible".to_string(), "true".to_string()),
                        ]),
                    }],
                },
                RepositoryRecordSet {
                    repository: CommerceOrderRecord::REPOSITORY.to_string(),
                    records: vec![RepositoryRecord {
                        id: "ORD-10042".to_string(),
                        fields: BTreeMap::from([
                            ("status".to_string(), "paid".to_string()),
                            ("payment_status".to_string(), "captured".to_string()),
                            ("payment_reference".to_string(), "PAY-50001".to_string()),
                            ("payment_method".to_string(), "card".to_string()),
                            (
                                "checkout_email".to_string(),
                                "buyer@example.com".to_string(),
                            ),
                            ("principal_id".to_string(), "member-live-1".to_string()),
                            ("currency".to_string(), "GBP".to_string()),
                            ("total_minor".to_string(), "12900".to_string()),
                            ("line_count".to_string(), "1".to_string()),
                        ]),
                    }],
                },
            ],
        };

        let product = repository
            .commerce_catalog_product("gold-membership")
            .unwrap()
            .unwrap();
        assert_eq!(product.handle, "gold-membership");
        assert_eq!(product.sku, "membership-gold");
        assert_eq!(product.price_minor, 12_900);

        let collection = repository
            .commerce_catalog_collection("memberships")
            .unwrap()
            .unwrap();
        assert_eq!(collection.handle, "memberships");
        assert!(collection.is_visible);

        let order = repository
            .commerce_order_by_payment_reference("PAY-50001")
            .unwrap()
            .unwrap();
        assert_eq!(order.order_id, "ORD-10042");
        assert_eq!(order.payment_status, "captured");
        assert_eq!(order.total_minor, 12_900);
    }
}
