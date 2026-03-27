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
pub use module::{
    CommerceModule, CommercePaymentsStripeConfig, CommercePaymentsStripeModule, StripeCheckoutMode,
    StripeProviderMetadata,
};
pub use orders::{Order, OrderOutcome, Refund};
pub use pricing::{DiscountRule, PriceAdjustment, PriceQuote, PricingPolicy};

pub fn module() -> CommerceModule {
    CommerceModule::new()
}

pub fn payments_stripe_module() -> CommercePaymentsStripeModule {
    CommercePaymentsStripeModule::new()
}

#[cfg(test)]
mod tests;
