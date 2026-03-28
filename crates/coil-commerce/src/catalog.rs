use crate::checkout::CheckoutLine;
use crate::error::CommerceModelError;
use crate::identifiers::{CollectionHandle, CollectionId, ProductHandle, ProductId, Sku};
use crate::model::{ProductKind, ProductStatus};
use crate::validation::require_non_empty;
use coil_data::{
    FilterOperator, PageRequest, PublicationVisibility, QueryCacheScope, QueryContext, QueryFilter,
    QuerySort, QuerySpec,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductVariant {
    pub sku: Sku,
    pub title: String,
    pub list_price: crate::model::Money,
}

impl ProductVariant {
    pub fn new(
        sku: Sku,
        title: impl Into<String>,
        list_price: crate::model::Money,
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
