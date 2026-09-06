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
