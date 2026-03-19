use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::time::Duration;

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
    DataModelError, DomainWrite, FilterOperator, MigrationId, MigrationOwner, MigrationPlan,
    MigrationStep, PageRequest, PublicationVisibility, QueryCacheScope, QueryContext, QueryField,
    QueryFilter, QuerySort, QuerySpec, RepositorySpec, TableName, TransactionIsolation,
    TransactionPlan,
};
use davenda_jobs::RetryPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommerceModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidRoute {
        field: &'static str,
        value: String,
    },
    DataPlan {
        error: DataModelError,
    },
    NegativeAmount {
        field: &'static str,
        amount_minor: i64,
    },
    ZeroQuantity {
        field: &'static str,
    },
    AmountOverflow {
        field: &'static str,
    },
    BasisPointsOutOfRange {
        field: &'static str,
        basis_points: u32,
    },
    CurrencyMismatch {
        expected: CurrencyCode,
        actual: CurrencyCode,
    },
    DuplicateVariant {
        sku: String,
    },
    MissingVariant {
        sku: String,
    },
    DuplicateProduct {
        product_id: String,
    },
    MissingProduct {
        product_id: String,
    },
    DuplicateCollection {
        collection_id: String,
    },
    MissingCollection {
        collection_id: String,
    },
    ProductNotSellable {
        product_id: String,
        status: ProductStatus,
    },
    MissingLine {
        sku: String,
    },
    EmptyCheckout,
    CheckoutNotReady {
        status: CheckoutStatus,
    },
    InvalidStatusTransition {
        from: CheckoutStatus,
        to: CheckoutStatus,
    },
    TotalWouldBecomeNegative {
        total_minor: i64,
    },
    OrderNotRefundable {
        order_id: String,
        status: OrderStatus,
    },
    RefundExceedsCaptured {
        order_id: String,
        captured_minor: i64,
        refunded_minor: i64,
        requested_minor: i64,
    },
}

impl fmt::Display for CommerceModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DataPlan { error } => write!(f, "{error}"),
            Self::NegativeAmount {
                field,
                amount_minor,
            } => write!(f, "`{field}` cannot be negative, got `{amount_minor}`"),
            Self::ZeroQuantity { field } => write!(f, "`{field}` must be greater than zero"),
            Self::AmountOverflow { field } => {
                write!(f, "arithmetic overflow while calculating `{field}`")
            }
            Self::BasisPointsOutOfRange {
                field,
                basis_points,
            } => {
                write!(
                    f,
                    "`{field}` must be between 0 and 10000, got `{basis_points}`"
                )
            }
            Self::CurrencyMismatch { expected, actual } => {
                write!(
                    f,
                    "currency mismatch: expected `{expected}`, got `{actual}`"
                )
            }
            Self::DuplicateVariant { sku } => write!(f, "variant `{sku}` is duplicated"),
            Self::MissingVariant { sku } => write!(f, "variant `{sku}` was not found"),
            Self::DuplicateProduct { product_id } => {
                write!(f, "catalog product `{product_id}` is duplicated")
            }
            Self::MissingProduct { product_id } => {
                write!(f, "catalog product `{product_id}` was not found")
            }
            Self::DuplicateCollection { collection_id } => {
                write!(f, "catalog collection `{collection_id}` is duplicated")
            }
            Self::MissingCollection { collection_id } => {
                write!(f, "catalog collection `{collection_id}` was not found")
            }
            Self::ProductNotSellable { product_id, status } => {
                write!(f, "product `{product_id}` is not sellable while `{status}`")
            }
            Self::MissingLine { sku } => write!(f, "checkout line for `{sku}` was not found"),
            Self::EmptyCheckout => f.write_str("checkout must contain at least one line"),
            Self::CheckoutNotReady { status } => {
                write!(f, "checkout cannot advance from status `{status}`")
            }
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "cannot transition checkout from `{from}` to `{to}`")
            }
            Self::TotalWouldBecomeNegative { total_minor } => {
                write!(f, "priced total would become negative: `{total_minor}`")
            }
            Self::OrderNotRefundable { order_id, status } => {
                write!(f, "order `{order_id}` cannot be refunded while `{status}`")
            }
            Self::RefundExceedsCaptured {
                order_id,
                captured_minor,
                refunded_minor,
                requested_minor,
            } => write!(
                f,
                "refund for order `{order_id}` exceeds captured amount: captured={captured_minor} refunded={refunded_minor} requested={requested_minor}"
            ),
        }
    }
}

impl Error for CommerceModelError {}

impl From<DataModelError> for CommerceModelError {
    fn from(error: DataModelError) -> Self {
        Self::DataPlan { error }
    }
}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, CommerceModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(ProductId, "product_id");
token_type!(ProductHandle, "product_handle");
token_type!(CollectionId, "collection_id");
token_type!(CollectionHandle, "collection_handle");
token_type!(OrderId, "order_id");
token_type!(CheckoutId, "checkout_id");
token_type!(RefundId, "refund_id");
token_type!(Sku, "sku");
token_type!(CurrencyCode, "currency");
token_type!(EntitlementKey, "entitlement_key");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductKind {
    Physical,
    Digital,
    Service,
    Membership { entitlement_key: EntitlementKey },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductStatus {
    Draft,
    Active,
    Archived,
}

impl fmt::Display for ProductStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::Active => f.write_str("active"),
            Self::Archived => f.write_str("archived"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckoutStatus {
    Draft,
    ReadyForPayment,
    AwaitingPayment,
    Paid,
    Completed,
    Cancelled,
}

impl fmt::Display for CheckoutStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => f.write_str("draft"),
            Self::ReadyForPayment => f.write_str("ready_for_payment"),
            Self::AwaitingPayment => f.write_str("awaiting_payment"),
            Self::Paid => f.write_str("paid"),
            Self::Completed => f.write_str("completed"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    PendingPayment,
    Paid,
    Fulfilled,
    PartiallyRefunded,
    Refunded,
    Cancelled,
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingPayment => f.write_str("pending_payment"),
            Self::Paid => f.write_str("paid"),
            Self::Fulfilled => f.write_str("fulfilled"),
            Self::PartiallyRefunded => f.write_str("partially_refunded"),
            Self::Refunded => f.write_str("refunded"),
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentKind {
    Promotion,
    Voucher,
    MembershipBenefit,
    Shipping,
    Tax,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdjustmentDirection {
    Discount,
    Surcharge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Money {
    currency: CurrencyCode,
    amount_minor: i64,
}

impl Money {
    pub fn new(currency: CurrencyCode, amount_minor: i64) -> Result<Self, CommerceModelError> {
        if amount_minor < 0 {
            return Err(CommerceModelError::NegativeAmount {
                field: "amount_minor",
                amount_minor,
            });
        }

        Ok(Self {
            currency,
            amount_minor,
        })
    }

    pub fn zero(currency: CurrencyCode) -> Self {
        Self {
            currency,
            amount_minor: 0,
        }
    }

    pub fn currency(&self) -> &CurrencyCode {
        &self.currency
    }

    pub fn amount_minor(&self) -> i64 {
        self.amount_minor
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, CommerceModelError> {
        ensure_same_currency(&self.currency, &other.currency)?;
        let amount = self.amount_minor.checked_add(other.amount_minor).ok_or(
            CommerceModelError::AmountOverflow {
                field: "money_addition",
            },
        )?;
        Self::new(self.currency.clone(), amount)
    }

    pub fn checked_mul(&self, quantity: u32) -> Result<Self, CommerceModelError> {
        let amount = self.amount_minor.checked_mul(i64::from(quantity)).ok_or(
            CommerceModelError::AmountOverflow {
                field: "money_multiplication",
            },
        )?;
        Self::new(self.currency.clone(), amount)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductVariant {
    pub sku: Sku,
    pub title: String,
    pub list_price: Money,
}

impl ProductVariant {
    pub fn new(
        sku: Sku,
        title: impl Into<String>,
        list_price: Money,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            sku,
            title: require_non_empty("variant_title", title.into())?,
            list_price,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProduct {
    pub id: ProductId,
    pub handle: ProductHandle,
    pub title: String,
    pub kind: ProductKind,
    pub status: ProductStatus,
    variants: BTreeMap<Sku, ProductVariant>,
}

impl CatalogProduct {
    pub fn new(
        id: ProductId,
        handle: ProductHandle,
        title: impl Into<String>,
        kind: ProductKind,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            id,
            handle,
            title: require_non_empty("product_title", title.into())?,
            kind,
            status: ProductStatus::Draft,
            variants: BTreeMap::new(),
        })
    }

    pub fn activate(mut self) -> Self {
        self.status = ProductStatus::Active;
        self
    }

    pub fn archive(mut self) -> Self {
        self.status = ProductStatus::Archived;
        self
    }

    pub fn with_variant(mut self, variant: ProductVariant) -> Result<Self, CommerceModelError> {
        if self.variants.contains_key(&variant.sku) {
            return Err(CommerceModelError::DuplicateVariant {
                sku: variant.sku.to_string(),
            });
        }

        self.variants.insert(variant.sku.clone(), variant);
        Ok(self)
    }

    pub fn variants(&self) -> impl Iterator<Item = &ProductVariant> {
        self.variants.values()
    }

    pub fn variant(&self, sku: &Sku) -> Result<&ProductVariant, CommerceModelError> {
        self.variants
            .get(sku)
            .ok_or_else(|| CommerceModelError::MissingVariant {
                sku: sku.to_string(),
            })
    }

    pub fn is_sellable(&self) -> bool {
        self.status == ProductStatus::Active && !self.variants.is_empty()
    }

    pub fn checkout_line(
        &self,
        sku: &Sku,
        quantity: u32,
    ) -> Result<CheckoutLine, CommerceModelError> {
        if self.status != ProductStatus::Active {
            return Err(CommerceModelError::ProductNotSellable {
                product_id: self.id.to_string(),
                status: self.status,
            });
        }

        let variant = self.variant(sku)?;
        CheckoutLine::new(
            self.id.clone(),
            self.kind.clone(),
            self.title.clone(),
            variant.sku.clone(),
            variant.title.clone(),
            quantity,
            variant.list_price.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogCollection {
    pub id: CollectionId,
    pub handle: CollectionHandle,
    pub title: String,
    product_ids: BTreeSet<ProductId>,
}

impl CatalogCollection {
    pub fn new(
        id: CollectionId,
        handle: CollectionHandle,
        title: impl Into<String>,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            id,
            handle,
            title: require_non_empty("collection_title", title.into())?,
            product_ids: BTreeSet::new(),
        })
    }

    pub fn include_product(mut self, product_id: ProductId) -> Self {
        self.product_ids.insert(product_id);
        self
    }

    pub fn product_ids(&self) -> &BTreeSet<ProductId> {
        &self.product_ids
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    products: BTreeMap<ProductId, CatalogProduct>,
    collections: BTreeMap<CollectionId, CatalogCollection>,
}

impl Catalog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_product(&mut self, product: CatalogProduct) -> Result<(), CommerceModelError> {
        if self.products.contains_key(&product.id) {
            return Err(CommerceModelError::DuplicateProduct {
                product_id: product.id.to_string(),
            });
        }

        self.products.insert(product.id.clone(), product);
        Ok(())
    }

    pub fn insert_collection(
        &mut self,
        collection: CatalogCollection,
    ) -> Result<(), CommerceModelError> {
        if self.collections.contains_key(&collection.id) {
            return Err(CommerceModelError::DuplicateCollection {
                collection_id: collection.id.to_string(),
            });
        }

        self.collections.insert(collection.id.clone(), collection);
        Ok(())
    }

    pub fn product(&self, id: &ProductId) -> Result<&CatalogProduct, CommerceModelError> {
        self.products
            .get(id)
            .ok_or_else(|| CommerceModelError::MissingProduct {
                product_id: id.to_string(),
            })
    }

    pub fn collection(&self, id: &CollectionId) -> Result<&CatalogCollection, CommerceModelError> {
        self.collections
            .get(id)
            .ok_or_else(|| CommerceModelError::MissingCollection {
                collection_id: id.to_string(),
            })
    }

    pub fn collection_products(
        &self,
        collection_id: &CollectionId,
    ) -> Result<Vec<&CatalogProduct>, CommerceModelError> {
        let collection = self.collection(collection_id)?;
        collection
            .product_ids()
            .iter()
            .map(|product_id| self.product(product_id))
            .collect()
    }

    pub fn storefront_listing_query(
        &self,
        locale: Option<&str>,
        collection_handle: Option<&CollectionHandle>,
    ) -> Result<CatalogListingQuery, CommerceModelError> {
        let mut query = QuerySpec::new(
            PageRequest::new(0, 24)?,
            QueryContext {
                locale: locale.map(str::to_owned),
                principal_id: None,
                publication_visibility: PublicationVisibility::PublishedOnly,
                cache_scope: if locale.is_some() {
                    QueryCacheScope::LocaleScoped
                } else {
                    QueryCacheScope::Public
                },
            },
        )
        .with_filter(QueryFilter::new(
            "catalog_status",
            FilterOperator::Eq,
            vec![ProductStatus::Active.to_string()],
        )?)
        .with_sort(QuerySort::ascending("product_title")?);

        if let Some(collection_handle) = collection_handle {
            query = query.with_filter(QueryFilter::new(
                "collection_handle",
                FilterOperator::Eq,
                vec![collection_handle.as_str().to_string()],
            )?);
        }

        Ok(CatalogListingQuery { query })
    }

    pub fn admin_catalog_query(
        &self,
        principal_id: &str,
        locale: Option<&str>,
    ) -> Result<CatalogListingQuery, CommerceModelError> {
        let query = QuerySpec::new(
            PageRequest::new(0, 50)?,
            QueryContext {
                locale: locale.map(str::to_owned),
                principal_id: Some(require_non_empty("principal_id", principal_id.to_string())?),
                publication_visibility: PublicationVisibility::IncludeDrafts,
                cache_scope: QueryCacheScope::UserScoped,
            },
        )
        .with_sort(QuerySort::ascending("product_title")?);

        Ok(CatalogListingQuery { query })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogListingQuery {
    pub query: QuerySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceAdjustment {
    pub kind: AdjustmentKind,
    pub direction: AdjustmentDirection,
    pub label: String,
    pub amount: Money,
}

impl PriceAdjustment {
    pub fn discount(
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            kind,
            direction: AdjustmentDirection::Discount,
            label: require_non_empty("adjustment_label", label.into())?,
            amount,
        })
    }

    pub fn surcharge(
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            kind,
            direction: AdjustmentDirection::Surcharge,
            label: require_non_empty("adjustment_label", label.into())?,
            amount,
        })
    }

    pub fn signed_minor_delta(&self) -> i64 {
        match self.direction {
            AdjustmentDirection::Discount => -self.amount.amount_minor(),
            AdjustmentDirection::Surcharge => self.amount.amount_minor(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscountRule {
    Percentage {
        kind: AdjustmentKind,
        label: String,
        basis_points: u32,
    },
    Fixed {
        kind: AdjustmentKind,
        label: String,
        amount: Money,
    },
}

impl DiscountRule {
    fn build_adjustment(&self, subtotal: &Money) -> Result<PriceAdjustment, CommerceModelError> {
        match self {
            Self::Percentage {
                kind,
                label,
                basis_points,
            } => {
                validate_basis_points("discount_basis_points", *basis_points)?;
                let amount = subtotal
                    .amount_minor()
                    .checked_mul(i64::from(*basis_points))
                    .ok_or(CommerceModelError::AmountOverflow {
                        field: "discount_amount",
                    })?
                    / 10_000;
                PriceAdjustment::discount(
                    *kind,
                    label.clone(),
                    Money::new(subtotal.currency().clone(), amount)?,
                )
            }
            Self::Fixed {
                kind,
                label,
                amount,
            } => PriceAdjustment::discount(*kind, label.clone(), amount.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PricingPolicy {
    pub currency: CurrencyCode,
    tax_rate_basis_points: u32,
    discounts: Vec<DiscountRule>,
    surcharges: Vec<PriceAdjustment>,
}

impl PricingPolicy {
    pub fn new(currency: CurrencyCode) -> Self {
        Self {
            currency,
            tax_rate_basis_points: 0,
            discounts: Vec::new(),
            surcharges: Vec::new(),
        }
    }

    pub fn with_membership_discount_basis_points(
        mut self,
        basis_points: u32,
    ) -> Result<Self, CommerceModelError> {
        validate_basis_points("membership_discount_basis_points", basis_points)?;
        self.discounts.push(DiscountRule::Percentage {
            kind: AdjustmentKind::MembershipBenefit,
            label: "Member pricing".to_string(),
            basis_points,
        });
        Ok(self)
    }

    pub fn with_fixed_discount(
        mut self,
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        ensure_same_currency(&self.currency, amount.currency())?;
        self.discounts.push(DiscountRule::Fixed {
            kind,
            label: require_non_empty("discount_label", label.into())?,
            amount,
        });
        Ok(self)
    }

    pub fn with_shipping(self, amount: Money) -> Result<Self, CommerceModelError> {
        self.with_surcharge(AdjustmentKind::Shipping, "Shipping", amount)
    }

    pub fn with_surcharge(
        mut self,
        kind: AdjustmentKind,
        label: impl Into<String>,
        amount: Money,
    ) -> Result<Self, CommerceModelError> {
        ensure_same_currency(&self.currency, amount.currency())?;
        self.surcharges
            .push(PriceAdjustment::surcharge(kind, label, amount)?);
        Ok(self)
    }

    pub fn with_tax_rate_basis_points(
        mut self,
        basis_points: u32,
    ) -> Result<Self, CommerceModelError> {
        validate_basis_points("tax_rate_basis_points", basis_points)?;
        self.tax_rate_basis_points = basis_points;
        Ok(self)
    }

    fn adjustments_for_subtotal(
        &self,
        subtotal: &Money,
    ) -> Result<Vec<PriceAdjustment>, CommerceModelError> {
        ensure_same_currency(&self.currency, subtotal.currency())?;

        let mut adjustments = Vec::new();
        for rule in &self.discounts {
            adjustments.push(rule.build_adjustment(subtotal)?);
        }
        adjustments.extend(self.surcharges.clone());

        let pre_tax_quote = PriceQuote::new(subtotal.clone(), adjustments.clone())?;
        if self.tax_rate_basis_points > 0 && pre_tax_quote.total.amount_minor() > 0 {
            let tax_minor = pre_tax_quote
                .total
                .amount_minor()
                .checked_mul(i64::from(self.tax_rate_basis_points))
                .ok_or(CommerceModelError::AmountOverflow {
                    field: "tax_amount",
                })?
                / 10_000;
            adjustments.push(PriceAdjustment::surcharge(
                AdjustmentKind::Tax,
                "Tax",
                Money::new(self.currency.clone(), tax_minor)?,
            )?);
        }

        Ok(adjustments)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceQuote {
    pub subtotal: Money,
    pub adjustments: Vec<PriceAdjustment>,
    pub total: Money,
}

impl PriceQuote {
    pub fn new(
        subtotal: Money,
        adjustments: Vec<PriceAdjustment>,
    ) -> Result<Self, CommerceModelError> {
        let mut total_minor = i128::from(subtotal.amount_minor());
        for adjustment in &adjustments {
            ensure_same_currency(subtotal.currency(), adjustment.amount.currency())?;
            total_minor += i128::from(adjustment.signed_minor_delta());
        }

        if total_minor < 0 {
            return Err(CommerceModelError::TotalWouldBecomeNegative {
                total_minor: total_minor.try_into().unwrap_or(i64::MIN),
            });
        }

        let total_minor: i64 =
            total_minor
                .try_into()
                .map_err(|_| CommerceModelError::AmountOverflow {
                    field: "priced_total",
                })?;

        Ok(Self {
            subtotal: subtotal.clone(),
            adjustments,
            total: Money::new(subtotal.currency().clone(), total_minor)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutLine {
    pub product_id: ProductId,
    pub product_kind: ProductKind,
    pub product_title: String,
    pub sku: Sku,
    pub variant_title: String,
    pub quantity: u32,
    pub unit_price: Money,
}

impl CheckoutLine {
    pub fn new(
        product_id: ProductId,
        product_kind: ProductKind,
        product_title: impl Into<String>,
        sku: Sku,
        variant_title: impl Into<String>,
        quantity: u32,
        unit_price: Money,
    ) -> Result<Self, CommerceModelError> {
        if quantity == 0 {
            return Err(CommerceModelError::ZeroQuantity { field: "quantity" });
        }

        Ok(Self {
            product_id,
            product_kind,
            product_title: require_non_empty("product_title", product_title.into())?,
            sku,
            variant_title: require_non_empty("variant_title", variant_title.into())?,
            quantity,
            unit_price,
        })
    }

    pub fn subtotal(&self) -> Result<Money, CommerceModelError> {
        self.unit_price.checked_mul(self.quantity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutSession {
    pub id: CheckoutId,
    pub currency: CurrencyCode,
    pub status: CheckoutStatus,
    lines: BTreeMap<Sku, CheckoutLine>,
}

impl CheckoutSession {
    pub fn new(id: CheckoutId, currency: CurrencyCode) -> Self {
        Self {
            id,
            currency,
            status: CheckoutStatus::Draft,
            lines: BTreeMap::new(),
        }
    }

    pub fn lines(&self) -> impl Iterator<Item = &CheckoutLine> {
        self.lines.values()
    }

    pub fn line(&self, sku: &Sku) -> Result<&CheckoutLine, CommerceModelError> {
        self.lines
            .get(sku)
            .ok_or_else(|| CommerceModelError::MissingLine {
                sku: sku.to_string(),
            })
    }

    pub fn add_line(&mut self, line: CheckoutLine) -> Result<(), CommerceModelError> {
        ensure_same_currency(&self.currency, line.unit_price.currency())?;
        if let Some(existing) = self.lines.get_mut(&line.sku) {
            existing.quantity = existing.quantity.checked_add(line.quantity).ok_or(
                CommerceModelError::AmountOverflow {
                    field: "checkout_line_quantity",
                },
            )?;
        } else {
            self.lines.insert(line.sku.clone(), line);
        }
        Ok(())
    }

    pub fn remove_line(&mut self, sku: &Sku) -> Option<CheckoutLine> {
        self.lines.remove(sku)
    }

    pub fn replace_quantity(&mut self, sku: &Sku, quantity: u32) -> Result<(), CommerceModelError> {
        if quantity == 0 {
            return Err(CommerceModelError::ZeroQuantity { field: "quantity" });
        }

        let line = self
            .lines
            .get_mut(sku)
            .ok_or_else(|| CommerceModelError::MissingLine {
                sku: sku.to_string(),
            })?;
        line.quantity = quantity;
        Ok(())
    }

    pub fn price(&self, policy: &PricingPolicy) -> Result<PriceQuote, CommerceModelError> {
        ensure_same_currency(&self.currency, &policy.currency)?;

        let mut subtotal = Money::zero(self.currency.clone());
        for line in self.lines() {
            subtotal = subtotal.checked_add(&line.subtotal()?)?;
        }

        let adjustments = policy.adjustments_for_subtotal(&subtotal)?;
        PriceQuote::new(subtotal, adjustments)
    }

    pub fn ready_for_payment(&mut self) -> Result<(), CommerceModelError> {
        if self.lines.is_empty() {
            return Err(CommerceModelError::EmptyCheckout);
        }

        self.transition_to(CheckoutStatus::ReadyForPayment)
    }

    pub fn awaiting_payment(&mut self) -> Result<(), CommerceModelError> {
        if self.status != CheckoutStatus::ReadyForPayment {
            return Err(CommerceModelError::CheckoutNotReady {
                status: self.status,
            });
        }

        self.transition_to(CheckoutStatus::AwaitingPayment)
    }

    pub fn mark_paid(&mut self) -> Result<(), CommerceModelError> {
        if self.status != CheckoutStatus::AwaitingPayment {
            return Err(CommerceModelError::CheckoutNotReady {
                status: self.status,
            });
        }

        self.transition_to(CheckoutStatus::Paid)
    }

    pub fn complete(&mut self) -> Result<(), CommerceModelError> {
        if self.status != CheckoutStatus::Paid {
            return Err(CommerceModelError::CheckoutNotReady {
                status: self.status,
            });
        }

        self.transition_to(CheckoutStatus::Completed)
    }

    pub fn cancel(&mut self) -> Result<(), CommerceModelError> {
        self.transition_to(CheckoutStatus::Cancelled)
    }

    pub fn to_order(
        &self,
        order_id: OrderId,
        pricing: &PricingPolicy,
    ) -> Result<Order, CommerceModelError> {
        if self.lines.is_empty() {
            return Err(CommerceModelError::EmptyCheckout);
        }

        let status = match self.status {
            CheckoutStatus::Paid | CheckoutStatus::Completed => OrderStatus::Paid,
            CheckoutStatus::Cancelled => OrderStatus::Cancelled,
            _ => OrderStatus::PendingPayment,
        };

        Ok(Order {
            id: order_id,
            status,
            currency: self.currency.clone(),
            lines: self.lines.values().cloned().collect(),
            totals: self.price(pricing)?,
            refunds: Vec::new(),
        })
    }

    pub fn completion_transaction_plan(
        &self,
        order: &Order,
    ) -> Result<TransactionPlan, CommerceModelError> {
        TransactionPlan::new(
            "commerce.checkout.complete",
            TransactionIsolation::Serializable,
        )?
        .with_write(DomainWrite::new("checkout_session", "update")?)
        .with_write(DomainWrite::new("checkout_line", "replace")?)
        .with_write(DomainWrite::new("order", "insert")?)
        .with_write(DomainWrite::new("inventory_reservation", "insert")?)
        .with_after_commit_job(format!("commerce.jobs.fulfillment.prepare:{}", order.id))
        .and_then(|plan| {
            plan.with_after_commit_event(format!("commerce.order.created:{}", order.id))
        })
        .map_err(Into::into)
    }

    fn transition_to(&mut self, next: CheckoutStatus) -> Result<(), CommerceModelError> {
        let valid = matches!(
            (self.status, next),
            (CheckoutStatus::Draft, CheckoutStatus::ReadyForPayment)
                | (
                    CheckoutStatus::ReadyForPayment,
                    CheckoutStatus::AwaitingPayment
                )
                | (CheckoutStatus::AwaitingPayment, CheckoutStatus::Paid)
                | (CheckoutStatus::Paid, CheckoutStatus::Completed)
                | (
                    CheckoutStatus::Draft
                        | CheckoutStatus::ReadyForPayment
                        | CheckoutStatus::AwaitingPayment
                        | CheckoutStatus::Paid,
                    CheckoutStatus::Cancelled
                )
        );

        if valid {
            self.status = next;
            Ok(())
        } else {
            Err(CommerceModelError::InvalidStatusTransition {
                from: self.status,
                to: next,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refund {
    pub id: RefundId,
    pub amount: Money,
    pub reason: String,
}

impl Refund {
    pub fn new(
        id: RefundId,
        amount: Money,
        reason: impl Into<String>,
    ) -> Result<Self, CommerceModelError> {
        Ok(Self {
            id,
            amount,
            reason: require_non_empty("refund_reason", reason.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderOutcome {
    ShipPhysical {
        sku: Sku,
        quantity: u32,
    },
    DeliverDigital {
        sku: Sku,
        quantity: u32,
    },
    ScheduleService {
        sku: Sku,
        quantity: u32,
    },
    GrantMembership {
        entitlement_key: EntitlementKey,
        quantity: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: OrderId,
    pub status: OrderStatus,
    pub currency: CurrencyCode,
    pub lines: Vec<CheckoutLine>,
    pub totals: PriceQuote,
    refunds: Vec<Refund>,
}

impl Order {
    pub fn refunds(&self) -> &[Refund] {
        &self.refunds
    }

    pub fn outcomes(&self) -> Vec<OrderOutcome> {
        self.lines
            .iter()
            .map(|line| match &line.product_kind {
                ProductKind::Physical => OrderOutcome::ShipPhysical {
                    sku: line.sku.clone(),
                    quantity: line.quantity,
                },
                ProductKind::Digital => OrderOutcome::DeliverDigital {
                    sku: line.sku.clone(),
                    quantity: line.quantity,
                },
                ProductKind::Service => OrderOutcome::ScheduleService {
                    sku: line.sku.clone(),
                    quantity: line.quantity,
                },
                ProductKind::Membership { entitlement_key } => OrderOutcome::GrantMembership {
                    entitlement_key: entitlement_key.clone(),
                    quantity: line.quantity,
                },
            })
            .collect()
    }

    pub fn fulfill(&mut self) -> Result<(), CommerceModelError> {
        if self.status != OrderStatus::Paid {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        self.status = OrderStatus::Fulfilled;
        Ok(())
    }

    pub fn issue_refund(&mut self, refund: Refund) -> Result<(), CommerceModelError> {
        if !matches!(
            self.status,
            OrderStatus::Paid | OrderStatus::Fulfilled | OrderStatus::PartiallyRefunded
        ) {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        ensure_same_currency(&self.currency, refund.amount.currency())?;

        let captured_minor = self.totals.total.amount_minor();
        let refunded_minor: i64 = self
            .refunds
            .iter()
            .map(|existing| existing.amount.amount_minor())
            .sum();
        let requested_minor = refund.amount.amount_minor();

        if refunded_minor + requested_minor > captured_minor {
            return Err(CommerceModelError::RefundExceedsCaptured {
                order_id: self.id.to_string(),
                captured_minor,
                refunded_minor,
                requested_minor,
            });
        }

        self.refunds.push(refund);
        let total_refunded = refunded_minor + requested_minor;
        self.status = if total_refunded == captured_minor {
            OrderStatus::Refunded
        } else {
            OrderStatus::PartiallyRefunded
        };
        Ok(())
    }

    pub fn fulfillment_transaction_plan(&self) -> Result<TransactionPlan, CommerceModelError> {
        if self.status != OrderStatus::Paid {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        TransactionPlan::new("commerce.order.fulfill", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("order", "update")?)
            .with_write(DomainWrite::new("fulfillment_job", "enqueue")?)
            .with_after_commit_job(format!("commerce.jobs.fulfillment.dispatch:{}", self.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!(
                    "commerce.order.fulfillment_requested:{}",
                    self.id
                ))
            })
            .map_err(Into::into)
    }

    pub fn refund_transaction_plan(
        &self,
        refund: &Refund,
    ) -> Result<TransactionPlan, CommerceModelError> {
        if !matches!(
            self.status,
            OrderStatus::Paid | OrderStatus::Fulfilled | OrderStatus::PartiallyRefunded
        ) {
            return Err(CommerceModelError::OrderNotRefundable {
                order_id: self.id.to_string(),
                status: self.status,
            });
        }

        TransactionPlan::new("commerce.order.refund", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("order_refund", "insert")?)
            .with_write(DomainWrite::new("order", "update")?)
            .with_write(DomainWrite::new("payment_refund", "request")?)
            .with_after_commit_job(format!("commerce.jobs.refund.reconcile:{}", refund.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!("commerce.order.refund_issued:{}", refund.id))
            })
            .map_err(Into::into)
    }
}

mod module;
pub use module::CommerceModule;

fn ensure_same_currency(
    expected: &CurrencyCode,
    actual: &CurrencyCode,
) -> Result<(), CommerceModelError> {
    if expected == actual {
        Ok(())
    } else {
        Err(CommerceModelError::CurrencyMismatch {
            expected: expected.clone(),
            actual: actual.clone(),
        })
    }
}

fn validate_basis_points(field: &'static str, basis_points: u32) -> Result<(), CommerceModelError> {
    if basis_points <= 10_000 {
        Ok(())
    } else {
        Err(CommerceModelError::BasisPointsOutOfRange {
            field,
            basis_points,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, CommerceModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CommerceModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, CommerceModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(CommerceModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

fn default_retry_policy() -> RetryPolicy {
    RetryPolicy::new(3, Duration::from_secs(15), Duration::from_secs(300))
        .expect("constant retry policy is valid")
}

#[cfg(test)]
mod tests;
