use coil::CoilRequestScope;
use coil::fission::core::{JobRef, JobSpec};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogRequest {
    pub scope: CoilRequestScope,
    pub collection: Option<String>,
    pub product: Option<String>,
    pub search: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogProduct {
    pub id: String,
    pub handle: String,
    pub sku: String,
    pub title: String,
    pub summary: String,
    pub price_minor: i64,
    pub currency: String,
    pub collection_handle: String,
    pub inventory_locations: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogCollection {
    pub id: String,
    pub handle: String,
    pub title: String,
    pub label: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogResponse {
    pub products: Vec<CatalogProduct>,
    pub collections: Vec<CatalogCollection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShopprJobError {
    pub code: String,
    pub message: String,
}

impl ShopprJobError {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            code: "catalog_unavailable".to_string(),
            message: message.into(),
        }
    }

    pub fn invalid(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ShopprJobError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ShopprJobError {}

#[derive(Debug)]
pub struct CatalogJob;

impl JobSpec for CatalogJob {
    type Request = CatalogRequest;
    type Ok = CatalogResponse;
    type Err = ShopprJobError;

    const NAME: &'static str = "shoppr.catalog.read";
}

pub const CATALOG_JOB: JobRef<CatalogJob> = JobRef::new(CatalogJob::NAME);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartRequest {
    pub scope: CoilRequestScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddCartItemRequest {
    pub product_handle: String,
    pub quantity: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartLine {
    pub product_id: String,
    pub product_handle: String,
    pub title: String,
    pub quantity: u32,
    pub unit_price_minor: i64,
    pub total_minor: i64,
    pub currency: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CartSnapshot {
    pub item_count: u32,
    pub subtotal_minor: i64,
    pub currency: String,
    pub lines: Vec<CartLine>,
}

#[derive(Debug)]
pub struct CartReadJob;

impl JobSpec for CartReadJob {
    type Request = CartRequest;
    type Ok = CartSnapshot;
    type Err = ShopprJobError;

    const NAME: &'static str = "shoppr.cart.read";
}

#[derive(Debug)]
pub struct AddCartItemJob;

impl JobSpec for AddCartItemJob {
    type Request = AddCartItemRequest;
    type Ok = CartSnapshot;
    type Err = ShopprJobError;

    const NAME: &'static str = "shoppr.cart.add";
}

pub const CART_READ_JOB: JobRef<CartReadJob> = JobRef::new(CartReadJob::NAME);
pub const ADD_CART_ITEM_JOB: JobRef<AddCartItemJob> = JobRef::new(AddCartItemJob::NAME);
