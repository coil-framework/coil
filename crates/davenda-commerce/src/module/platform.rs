use super::core::CommerceModule;
use super::support;
use crate::pricing::default_retry_policy;
use davenda_auth::Capability;
use davenda_core::{
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind, IntegrationPoint,
    JobContract, JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency,
    ModuleManifest, PlatformModule, RegistrationError, ReportDefinition, ReportDeliveryMode,
    ReportFormat, ReportSensitivity, RouteSurface, RouteSurfaceKind, SearchDocumentKind,
    SearchFieldContribution, SearchFieldRole, SearchIndexContribution, SearchInvalidationRule,
    SearchInvalidationTrigger, SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use davenda_data::MigrationPlan;

impl PlatformModule for CommerceModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name().to_string())
            .with_required_capabilities(vec![
                Capability::CatalogProductRead,
                Capability::CatalogProductEdit,
                Capability::CatalogCollectionEdit,
                Capability::CheckoutSessionCreate,
                Capability::OrderRead,
                Capability::OrderRefundIssue,
            ])
            .with_optional_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::SeoMetadataEdit,
                Capability::I18nTranslationEdit,
                Capability::AssetRead,
            ])
            .with_config_namespace(self.config_namespace().to_string())
            .with_capability_contracts(vec![
                CapabilityContract::required(
                    Capability::CatalogProductRead,
                    ["product", "collection"],
                ),
                CapabilityContract::required(Capability::CatalogProductEdit, ["product"]),
                CapabilityContract::required(Capability::CatalogCollectionEdit, ["collection"]),
                CapabilityContract::required(Capability::CheckoutSessionCreate, ["storefront"]),
                CapabilityContract::required(Capability::OrderRead, ["order"]),
                CapabilityContract::required(Capability::OrderRefundIssue, ["order"]),
                CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
                CapabilityContract::optional(Capability::SeoMetadataEdit, ["product", "collection"]),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["product", "collection"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::optional(
                    "admin",
                    "Commerce contributes catalog and order operations into the shared admin shell",
                ),
                ModuleDependency::optional(
                    "cms",
                    "Storefront catalog pages can participate in CMS-driven content composition",
                ),
                ModuleDependency::optional(
                    "media",
                    "Products and collections can reference managed assets from the media library",
                ),
                ModuleDependency::optional(
                    "memberships",
                    "Membership products can materialize ongoing subscription state from commerce orders",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Cache,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Storage,
                CoreServiceDependency::I18n,
                CoreServiceDependency::Seo,
                CoreServiceDependency::Template,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "commerce.catalog",
                    10,
                    "Creates product, variant, and collection tables with localized merchandising fields",
                ),
                MigrationContract::new(
                    "commerce.checkout",
                    20,
                    "Creates checkout session state, captured pricing snapshots, and recovery tokens",
                ),
                MigrationContract::new(
                    "commerce.orders",
                    30,
                    "Creates order lifecycle, refund, and after-commit integration outbox tables",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new("commerce.catalog", RouteSurfaceKind::FrontendPage, "/shop")
                    .localized(),
                RouteSurface::new(
                    "commerce.checkout",
                    RouteSurfaceKind::FrontendAction,
                    "/checkout",
                )
                .gated_by(Capability::CheckoutSessionCreate),
                RouteSurface::new("commerce.orders", RouteSurfaceKind::AdminPage, "/admin/orders")
                    .gated_by(Capability::OrderRead),
                RouteSurface::new(
                    "commerce.catalog-admin",
                    RouteSurfaceKind::AdminPage,
                    "/admin/catalog/products",
                )
                .gated_by(Capability::CatalogProductEdit),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "commerce.order-confirmation",
                    JobTriggerKind::DomainEvent,
                    true,
                    "Schedules post-purchase confirmations and downstream fulfillment follow-up work",
                ),
                JobContract::new(
                    "commerce.refund-followup",
                    JobTriggerKind::Operator,
                    true,
                    "Completes asynchronous refund side effects and customer notification flows",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "commerce.order.paid",
                    Some("commerce.order-confirmation"),
                    "Launches order confirmation and post-checkout workflows after successful payment",
                ),
                EventSubscription::new(
                    "commerce.order.refund-issued",
                    Some("commerce.refund-followup"),
                    "Follows through on asynchronous refund side effects and reporting updates",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::FrontendRendering,
                    "storefront.catalog",
                    "Provides catalog, product, and checkout surfaces for customer storefronts",
                ),
                IntegrationPoint::new(
                    IntegrationKind::SearchIndex,
                    "catalog.index",
                    "Publishes searchable product and collection visibility data for first-party search",
                ),
                IntegrationPoint::new(
                    IntegrationKind::SeoMetadata,
                    "product.head",
                    "Emits canonical metadata and rich-result product schema for catalog surfaces",
                ),
                IntegrationPoint::new(
                    IntegrationKind::CacheInvalidation,
                    "catalog.publish",
                    "Invalidates product, collection, and merchandising fragments after catalog changes",
                ),
                IntegrationPoint::new(
                    IntegrationKind::CommerceBridge,
                    "membership.provisioning",
                    "Projects qualifying orders into membership and entitlement workflows when installed",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::CacheInvalidation,
                ModuleBehavior::LocalizedContent,
                ModuleBehavior::SeoMetadata,
                ModuleBehavior::JsonLd,
                ModuleBehavior::AsyncJobs,
            ])
            .with_extension_slots(vec![
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::RenderHook,
                    "commerce.pricing",
                    "Allows bounded pricing and merchandising adjustments during storefront rendering",
                ),
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::Webhook,
                    "commerce.payment-provider",
                    "Allows payment-provider integrations to enter the system through explicit webhook contracts",
                ),
            ])
            .with_admin_resources(self.admin_resources().to_vec())
            .with_search_contributions(vec![
                SearchIndexContribution::new(
                    "search.catalog.products",
                    SearchDocumentKind::Product,
                    SearchVisibility::Public,
                    true,
                    vec![
                        SearchFieldContribution::new(
                            "name",
                            "name",
                            SearchFieldRole::Title,
                            true,
                            true,
                        ),
                        SearchFieldContribution::new(
                            "description",
                            "description",
                            SearchFieldRole::Body,
                            false,
                            true,
                        ),
                        SearchFieldContribution::new(
                            "sku",
                            "variants.sku",
                            SearchFieldRole::Keyword,
                            true,
                            true,
                        ),
                    ],
                    vec![
                        SearchInvalidationRule::new(
                            SearchInvalidationTrigger::Published,
                            "product published",
                        ),
                        SearchInvalidationRule::new(
                            SearchInvalidationTrigger::Updated,
                            "product updated",
                        ),
                    ],
                    SearchRebuildStrategy::OnInvalidate,
                ),
                SearchIndexContribution::new(
                    "search.catalog.collections",
                    SearchDocumentKind::Collection,
                    SearchVisibility::Public,
                    true,
                    vec![
                        SearchFieldContribution::new(
                            "title",
                            "title",
                            SearchFieldRole::Title,
                            true,
                            true,
                        ),
                        SearchFieldContribution::new(
                            "summary",
                            "summary",
                            SearchFieldRole::Summary,
                            true,
                            true,
                        ),
                    ],
                    vec![
                        SearchInvalidationRule::new(
                            SearchInvalidationTrigger::Published,
                            "collection published",
                        ),
                        SearchInvalidationRule::new(
                            SearchInvalidationTrigger::Updated,
                            "collection updated",
                        ),
                    ],
                    SearchRebuildStrategy::OnInvalidate,
                ),
            ])
            .with_report_definitions(vec![ReportDefinition::new(
                "report.orders.summary",
                "Orders summary",
                Some("Operational summary of captured orders and refunds".to_string()),
                Capability::OrderRead,
                ReportFormat::Csv,
                ReportSensitivity::Restricted,
                ReportDeliveryMode::InternalOnly,
                "reports/orders",
                default_retry_policy(),
            )])
            .with_data_repositories(vec![support::commerce_catalog_products_repository()])
            .with_http_surfaces(vec![
                HttpSurfaceContribution::page(
                    "commerce.catalog",
                    HttpSurfaceArea::Public,
                    "/shop",
                    "commerce/catalog",
                )
                .localized(),
                HttpSurfaceContribution::page(
                    "commerce.checkout",
                    HttpSurfaceArea::Public,
                    "/checkout",
                    "commerce/checkout",
                )
                .gated_by(Capability::CheckoutSessionCreate),
                HttpSurfaceContribution::page(
                    "commerce.orders",
                    HttpSurfaceArea::Admin,
                    "/admin/orders",
                    "commerce/orders",
                )
                .gated_by(Capability::OrderRead),
                HttpSurfaceContribution::page(
                    "commerce.catalog-admin",
                    HttpSurfaceArea::Admin,
                    "/admin/catalog/products",
                    "commerce/catalog-admin",
                )
                .gated_by(Capability::CatalogProductEdit),
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name().to_string(),
            "module.commerce.catalog",
            "Catalog products, variants, and collection management",
        )?;
        registry.register_module_service(
            self.name().to_string(),
            "module.commerce.pricing",
            "Pricing policies, discounts, voucher application, and tax/shipping calculation",
        )?;
        registry.register_module_service(
            self.name().to_string(),
            "module.commerce.checkout",
            "Checkout sessions, payment readiness, and order materialization",
        )?;
        registry.register_module_service(
            self.name().to_string(),
            "module.commerce.orders",
            "Order lifecycle, fulfillment outcomes, and refund handling",
        )?;
        registry.register_module_service(
            self.name().to_string(),
            "module.commerce.membership_bridge",
            "Membership-aware product kinds and order outcomes for entitlement modules",
        )?;
        registry.register_module_service(
            self.name().to_string(),
            "module.commerce.admin",
            "Commerce admin resources, catalog operations, and order review",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        Some(
            CommerceModule::migration_plan(self)
                .expect("commerce migration plan is constant and valid"),
        )
    }
}
