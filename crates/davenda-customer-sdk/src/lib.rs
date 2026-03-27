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
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RecordingRegistry {
        hook_kinds: Vec<RegisteredHookKind>,
    }

    struct RecordingRepository {
        read_results: Vec<RepositoryRecordSet>,
        writes: Arc<Mutex<Vec<RepositoryWrite>>>,
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
            let change = _change;
            self.writes.lock().unwrap().push(change.clone());
            Ok(RepositoryWriteReceipt {
                repository: change.repository,
                record_id: change.record_id,
                version: Some("test-version".to_string()),
            })
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
                    repository: CmsPageRecord::REPOSITORY.to_string(),
                    records: vec![RepositoryRecord {
                        id: "page-membership-guide".to_string(),
                        fields: BTreeMap::from([
                            ("title".to_string(), "Membership guide".to_string()),
                            ("slug".to_string(), "membership-guide".to_string()),
                            ("summary".to_string(), "How activation works".to_string()),
                            ("body_html".to_string(), "<p>Body</p>".to_string()),
                            ("status".to_string(), "draft".to_string()),
                            (
                                "live_path".to_string(),
                                "/pages/membership-guide".to_string(),
                            ),
                        ]),
                    }],
                },
                RepositoryRecordSet {
                    repository: CmsNavigationRecord::REPOSITORY.to_string(),
                    records: vec![RepositoryRecord {
                        id: "0".to_string(),
                        fields: BTreeMap::from([
                            ("label".to_string(), "Home".to_string()),
                            ("href".to_string(), "/".to_string()),
                        ]),
                    }],
                },
                RepositoryRecordSet {
                    repository: CmsRedirectRecord::REPOSITORY.to_string(),
                    records: vec![RepositoryRecord {
                        id: "1".to_string(),
                        fields: BTreeMap::from([
                            ("from".to_string(), "/legacy".to_string()),
                            ("to".to_string(), "/pages/membership-guide".to_string()),
                            ("permanent".to_string(), "true".to_string()),
                        ]),
                    }],
                },
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
            writes: Arc::new(Mutex::new(Vec::new())),
        };

        let page = repository
            .cms_page("page-membership-guide")
            .unwrap()
            .unwrap();
        assert_eq!(page.page_id, "page-membership-guide");
        assert_eq!(page.slug, "membership-guide");
        assert_eq!(page.live_path.as_deref(), Some("/pages/membership-guide"));

        let navigation = repository.cms_navigation_items().unwrap();
        assert_eq!(navigation.len(), 1);
        assert_eq!(navigation[0].record_id, 0);
        assert_eq!(navigation[0].href, "/");

        let redirects = repository.cms_redirects().unwrap();
        assert_eq!(redirects.len(), 1);
        assert_eq!(redirects[0].record_id, 1);
        assert!(redirects[0].permanent);

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

        repository
            .update_cms_page(&CmsPageUpdate::new(
                "page-membership-guide",
                "Updated title",
                "membership-guide",
                "Updated summary",
                "<p>Updated body</p>",
            ))
            .unwrap();
        repository
            .append_cms_navigation_item(&CmsNavigationAppend::new(
                "Shipping",
                "/pages/shipping-returns",
            ))
            .unwrap();
        repository
            .append_cms_redirect(&CmsRedirectAppend::new(
                "/legacy/shipping",
                "/pages/shipping-returns",
                true,
            ))
            .unwrap();

        let writes = repository.writes.lock().unwrap().clone();
        assert_eq!(writes.len(), 3);
        assert_eq!(writes[0].repository, "cms.pages");
        assert_eq!(writes[0].record_id, "page-membership-guide");
        assert_eq!(
            writes[0].fields.get("title").map(String::as_str),
            Some("Updated title")
        );
        assert_eq!(writes[1].repository, "cms.navigation");
        assert_eq!(writes[1].record_id, "append");
        assert_eq!(
            writes[1].fields.get("href").map(String::as_str),
            Some("/pages/shipping-returns")
        );
        assert_eq!(writes[2].repository, "cms.redirects");
        assert_eq!(writes[2].record_id, "append");
        assert_eq!(
            writes[2].fields.get("permanent").map(String::as_str),
            Some("true")
        );
    }
}
