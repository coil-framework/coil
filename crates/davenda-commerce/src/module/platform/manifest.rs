use super::super::core::CommerceModule;
use super::super::support;
use crate::pricing::default_retry_policy;
use davenda_auth::Capability;
use davenda_core::{
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind, IntegrationPoint,
    JobContract, JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency,
    ModuleManifest, ReportDefinition, ReportDeliveryMode, ReportFormat, ReportSensitivity,
    RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution, SearchFieldRole,
    SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility,
};

pub(super) fn build_manifest(module: &CommerceModule) -> ModuleManifest {
    ModuleManifest::new(module.name().to_string())
        .with_required_capabilities(required_capabilities())
        .with_optional_capabilities(optional_capabilities())
        .with_config_namespace(module.config_namespace().to_string())
        .with_capability_contracts(capability_contracts())
        .with_module_dependencies(module_dependencies())
        .with_core_service_dependencies(core_service_dependencies())
        .with_migrations(module_migrations())
        .with_route_surfaces(route_surfaces())
        .with_jobs(jobs())
        .with_event_subscriptions(event_subscriptions())
        .with_integration_points(integration_points())
        .with_behaviors(module_behaviors())
        .with_extension_slots(extension_slots())
        .with_admin_resources(module.admin_resources().to_vec())
        .with_search_contributions(search_contributions())
        .with_report_definitions(report_definitions())
        .with_data_repositories(vec![support::commerce_catalog_products_repository()])
        .with_http_surfaces(http_surfaces())
}

fn required_capabilities() -> Vec<Capability> {
    vec![
        Capability::CatalogProductRead,
        Capability::CatalogProductEdit,
        Capability::CatalogCollectionEdit,
        Capability::CheckoutSessionCreate,
        Capability::OrderRead,
        Capability::OrderRefundIssue,
    ]
}

fn optional_capabilities() -> Vec<Capability> {
    vec![
        Capability::AdminShellAccess,
        Capability::SeoMetadataEdit,
        Capability::I18nTranslationEdit,
        Capability::AssetRead,
    ]
}

fn capability_contracts() -> Vec<CapabilityContract> {
    vec![
        CapabilityContract::required(Capability::CatalogProductRead, ["product", "collection"]),
        CapabilityContract::required(Capability::CatalogProductEdit, ["product"]),
        CapabilityContract::required(Capability::CatalogCollectionEdit, ["collection"]),
        CapabilityContract::required(Capability::CheckoutSessionCreate, ["storefront"]),
        CapabilityContract::required(Capability::OrderRead, ["order"]),
        CapabilityContract::required(Capability::OrderRefundIssue, ["order"]),
        CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
        CapabilityContract::optional(Capability::SeoMetadataEdit, ["product", "collection"]),
        CapabilityContract::optional(Capability::I18nTranslationEdit, ["product", "collection"]),
        CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
    ]
}

fn module_dependencies() -> Vec<ModuleDependency> {
    vec![
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
    ]
}

fn core_service_dependencies() -> Vec<CoreServiceDependency> {
    vec![
        CoreServiceDependency::Auth,
        CoreServiceDependency::Data,
        CoreServiceDependency::Cache,
        CoreServiceDependency::Jobs,
        CoreServiceDependency::Storage,
        CoreServiceDependency::I18n,
        CoreServiceDependency::Seo,
        CoreServiceDependency::Template,
        CoreServiceDependency::Observability,
    ]
}

fn module_migrations() -> Vec<MigrationContract> {
    vec![
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
    ]
}

fn route_surfaces() -> Vec<RouteSurface> {
    vec![
        RouteSurface::new("commerce.catalog", RouteSurfaceKind::FrontendPage, "/shop").localized(),
        RouteSurface::new(
            "commerce.collection-detail",
            RouteSurfaceKind::FrontendPage,
            "/shop/collections/{collection_slug}",
        )
        .localized(),
        RouteSurface::new(
            "commerce.product-detail",
            RouteSurfaceKind::FrontendPage,
            "/shop/products/{product_slug}",
        )
        .localized(),
        RouteSurface::new("commerce.cart", RouteSurfaceKind::FrontendPage, "/cart")
            .gated_by(Capability::CheckoutSessionCreate),
        RouteSurface::new(
            "commerce.checkout",
            RouteSurfaceKind::FrontendPage,
            "/checkout",
        )
        .gated_by(Capability::CheckoutSessionCreate),
        RouteSurface::new(
            "commerce.checkout-confirmation",
            RouteSurfaceKind::FrontendPage,
            "/checkout/confirmation",
        )
        .gated_by(Capability::CheckoutSessionCreate),
        RouteSurface::new(
            "commerce.orders",
            RouteSurfaceKind::AdminPage,
            "/admin/orders",
        )
        .gated_by(Capability::OrderRead),
        RouteSurface::new(
            "commerce.catalog-admin",
            RouteSurfaceKind::AdminPage,
            "/admin/catalog/products",
        )
        .gated_by(Capability::CatalogProductEdit),
    ]
}

fn jobs() -> Vec<JobContract> {
    vec![
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
    ]
}

fn event_subscriptions() -> Vec<EventSubscription> {
    vec![
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
    ]
}

fn integration_points() -> Vec<IntegrationPoint> {
    vec![
        IntegrationPoint::new(
            IntegrationKind::FrontendRendering,
            "storefront.catalog",
            "Provides collection listing/detail, product detail, cart, and checkout surfaces for customer storefronts",
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
    ]
}

fn module_behaviors() -> Vec<ModuleBehavior> {
    vec![
        ModuleBehavior::CacheInvalidation,
        ModuleBehavior::LocalizedContent,
        ModuleBehavior::SeoMetadata,
        ModuleBehavior::JsonLd,
        ModuleBehavior::AsyncJobs,
    ]
}

fn extension_slots() -> Vec<ExtensionSlotDescriptor> {
    vec![
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
    ]
}

fn search_contributions() -> Vec<SearchIndexContribution> {
    vec![
        SearchIndexContribution::new(
            "search.catalog.products",
            SearchDocumentKind::Product,
            SearchVisibility::Public,
            true,
            vec![
                SearchFieldContribution::new("name", "name", SearchFieldRole::Title, true, true),
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
                SearchInvalidationRule::new(SearchInvalidationTrigger::Updated, "product updated"),
            ],
            SearchRebuildStrategy::OnInvalidate,
        ),
        SearchIndexContribution::new(
            "search.catalog.collections",
            SearchDocumentKind::Collection,
            SearchVisibility::Public,
            true,
            vec![
                SearchFieldContribution::new("title", "title", SearchFieldRole::Title, true, true),
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
    ]
}

fn report_definitions() -> Vec<ReportDefinition> {
    vec![ReportDefinition::new(
        "report.orders.summary",
        "Orders summary",
        Some("Operational summary of captured orders and refunds".to_string()),
        Capability::OrderRead,
        ReportFormat::Csv,
        ReportSensitivity::Restricted,
        ReportDeliveryMode::InternalOnly,
        "reports/orders",
        default_retry_policy(),
    )]
}

fn http_surfaces() -> Vec<HttpSurfaceContribution> {
    vec![
        HttpSurfaceContribution::page(
            "commerce.catalog",
            HttpSurfaceArea::Public,
            "/shop",
            "commerce/catalog",
        )
        .localized(),
        HttpSurfaceContribution::page(
            "commerce.collection-detail",
            HttpSurfaceArea::Public,
            "/shop/collections/{collection_slug}",
            "commerce/collection-detail",
        )
        .localized(),
        HttpSurfaceContribution::page(
            "commerce.product-detail",
            HttpSurfaceArea::Public,
            "/shop/products/{product_slug}",
            "commerce/product-detail",
        )
        .localized(),
        HttpSurfaceContribution::page(
            "commerce.cart",
            HttpSurfaceArea::Public,
            "/cart",
            "commerce/cart",
        )
        .gated_by(Capability::CheckoutSessionCreate),
        HttpSurfaceContribution::page(
            "commerce.checkout",
            HttpSurfaceArea::Public,
            "/checkout",
            "commerce/checkout",
        )
        .gated_by(Capability::CheckoutSessionCreate),
        HttpSurfaceContribution::page(
            "commerce.checkout-confirmation",
            HttpSurfaceArea::Public,
            "/checkout/confirmation",
            "commerce/checkout-confirmation",
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
    ]
}
