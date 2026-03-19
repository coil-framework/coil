use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution, CapabilityContract,
    CoreServiceDependency, DataRepositoryContribution, DataRepositoryQueryProfile,
    EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind, HttpSurfaceArea,
    HttpSurfaceContribution, IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind,
    MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, ReportDefinition, ReportDeliveryMode, ReportFormat, ReportSensitivity,
    RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution, SearchFieldRole,
    SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use davenda_data::{
    MigrationId, MigrationOwner, MigrationPlan, MigrationStep, PageRequest, PublicationVisibility,
    QueryCacheScope, QueryField, QuerySort, RepositorySpec, TableName,
};
mod catalog;
mod checkout;
mod error;
mod identifiers;
mod model;
mod module;
mod orders;
mod pricing;
mod validation;

pub use catalog::{
    Catalog, CatalogCollection, CatalogListingQuery, CatalogProduct, ProductVariant,
};
pub use checkout::{CheckoutLine, CheckoutSession};
pub use error::CommerceModelError;
pub use identifiers::{
    CheckoutId, CollectionHandle, CollectionId, CurrencyCode, EntitlementKey, OrderId,
    ProductHandle, ProductId, RefundId, Sku,
};
pub use model::{
    AdjustmentDirection, AdjustmentKind, CheckoutStatus, Money, OrderStatus, ProductKind,
    ProductStatus,
};
pub use module::CommerceModule;
pub use orders::{Order, OrderOutcome, Refund};
pub use pricing::{DiscountRule, PriceAdjustment, PriceQuote, PricingPolicy};

#[cfg(test)]
mod tests;
