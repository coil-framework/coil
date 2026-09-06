use super::model::{
    CatalogCollection, CatalogProduct, CatalogRequest, CatalogResponse, ShopprJobError,
};
use coil_config::SiteConfig;
use coil_data::{DataModelError, DataRuntime, PostgresDataClient};
use coil_runtime::StorefrontCatalog;
use sqlx::Row;

#[derive(Clone)]
pub struct PostgresCatalogRepository {
    client: PostgresDataClient,
    schema: String,
}

impl PostgresCatalogRepository {
    pub fn connect(data: &DataRuntime) -> Result<Self, DataModelError> {
        Ok(Self {
            client: data.connect_lazy_postgres()?,
            schema: quote_identifier(&data.schema),
        })
    }

    pub async fn load(&self, request: CatalogRequest) -> Result<CatalogResponse, ShopprJobError> {
        let products = sqlx::query(&format!(
            r#"
            SELECT
                product.id,
                product.slug,
                product.sku,
                product.title,
                publication.summary,
                product.price_minor,
                product.currency,
                collection.handle AS collection_handle,
                COALESCE(
                    ARRAY_AGG(inventory.location_id ORDER BY inventory.location_id)
                        FILTER (WHERE inventory.is_available AND inventory.location_id IS NOT NULL),
                    ARRAY[]::TEXT[]
                ) AS inventory_locations
            FROM {schema}.commerce_catalog_products AS product
            JOIN {schema}.commerce_product_publications AS publication
              ON publication.product_id = product.id
            LEFT JOIN {schema}.commerce_collection_products AS membership
              ON membership.product_id = product.id
            LEFT JOIN {schema}.commerce_collections AS collection
              ON collection.id = membership.collection_id
            LEFT JOIN {schema}.commerce_inventory_locations AS inventory
              ON inventory.product_id = product.id
            WHERE product.status = 'active'
              AND publication.is_published
              AND publication.site_id = $1
              AND publication.locale = $2
              AND ($3::TEXT IS NULL OR collection.handle = $3)
              AND ($4::TEXT IS NULL OR product.slug = $4)
              AND (
                    $5::TEXT IS NULL
                    OR product.title ILIKE '%' || $5 || '%'
                    OR publication.summary ILIKE '%' || $5 || '%'
              )
            GROUP BY
                product.id,
                product.slug,
                product.sku,
                product.title,
                publication.summary,
                product.price_minor,
                product.currency,
                collection.handle
            ORDER BY product.title, product.id
            "#,
            schema = self.schema,
        ))
        .bind(&request.scope.site_id)
        .bind(&request.scope.locale)
        .bind(request.collection.as_deref())
        .bind(request.product.as_deref())
        .bind(request.search.as_deref())
        .fetch_all(&self.client.pool)
        .await
        .map_err(database_error)?
        .into_iter()
        .map(|row| {
            Ok(CatalogProduct {
                id: row.try_get("id").map_err(database_error)?,
                handle: row.try_get("slug").map_err(database_error)?,
                sku: row.try_get("sku").map_err(database_error)?,
                title: row.try_get("title").map_err(database_error)?,
                summary: row.try_get("summary").map_err(database_error)?,
                price_minor: row.try_get("price_minor").map_err(database_error)?,
                currency: row.try_get("currency").map_err(database_error)?,
                collection_handle: row
                    .try_get::<Option<String>, _>("collection_handle")
                    .map_err(database_error)?
                    .unwrap_or_default(),
                inventory_locations: row.try_get("inventory_locations").map_err(database_error)?,
            })
        })
        .collect::<Result<Vec<_>, ShopprJobError>>()?;

        let collections = sqlx::query(&format!(
            r#"
            SELECT
                collection.id,
                collection.handle,
                collection.title,
                publication.label,
                publication.summary
            FROM {schema}.commerce_collections AS collection
            JOIN {schema}.commerce_collection_publications AS publication
              ON publication.collection_id = collection.id
            WHERE collection.status = 'active'
              AND publication.is_published
              AND publication.site_id = $1
              AND publication.locale = $2
              AND ($3::TEXT IS NULL OR collection.handle = $3)
            ORDER BY collection.title, collection.id
            "#,
            schema = self.schema,
        ))
        .bind(&request.scope.site_id)
        .bind(&request.scope.locale)
        .bind(request.collection.as_deref())
        .fetch_all(&self.client.pool)
        .await
        .map_err(database_error)?
        .into_iter()
        .map(|row| {
            Ok(CatalogCollection {
                id: row.try_get("id").map_err(database_error)?,
                handle: row.try_get("handle").map_err(database_error)?,
                title: row.try_get("title").map_err(database_error)?,
                label: row.try_get("label").map_err(database_error)?,
                summary: row.try_get("summary").map_err(database_error)?,
            })
        })
        .collect::<Result<Vec<_>, ShopprJobError>>()?;

        Ok(CatalogResponse {
            products,
            collections,
        })
    }

    pub async fn seed_if_empty(
        &self,
        catalog: &StorefrontCatalog,
        sites: &[SiteConfig],
    ) -> Result<(), ShopprJobError> {
        let mut transaction = self.client.pool.begin().await.map_err(database_error)?;
        let now = unix_timestamp();

        for collection in &catalog.collections {
            let collection_id = format!("collection:{}", collection.handle);
            sqlx::query(&format!(
                "INSERT INTO {schema}.commerce_collections (id, handle, title, status, fingerprint, updated_at) VALUES ($1, $2, $3, 'active', 'shoppr-managed', $4) ON CONFLICT (id) DO NOTHING",
                schema = self.schema,
            ))
            .bind(&collection_id)
            .bind(&collection.handle)
            .bind(&collection.title)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;

            for site in sites.iter().filter(|site| {
                collection.site_ids.is_empty() || collection.site_ids.contains(&site.id)
            }) {
                for locale in &site.supported_locales {
                    sqlx::query(&format!(
                        "INSERT INTO {schema}.commerce_collection_publications (collection_id, site_id, locale, label, summary, is_published, updated_at) VALUES ($1, $2, $3, $4, $5, TRUE, $6) ON CONFLICT (collection_id, site_id, locale) DO NOTHING",
                        schema = self.schema,
                    ))
                    .bind(&collection_id)
                    .bind(&site.id)
                    .bind(locale)
                    .bind(&collection.label)
                    .bind(&collection.summary)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await
                    .map_err(database_error)?;
                }
            }
        }

        for product in &catalog.products {
            let product_id = format!("product:{}", product.handle);
            let collection_id = format!("collection:{}", product.collection_handle);
            sqlx::query(&format!(
                "INSERT INTO {schema}.commerce_catalog_products (id, slug, sku, title, product_type, status, price_minor, currency, fingerprint, updated_at) VALUES ($1, $2, $3, $4, $5, 'active', $6, $7, 'shoppr-managed', $8) ON CONFLICT (id) DO NOTHING",
                schema = self.schema,
            ))
            .bind(&product_id)
            .bind(&product.handle)
            .bind(&product.sku)
            .bind(&product.title)
            .bind(&product.product_kind)
            .bind(product.price_minor)
            .bind(&product.currency)
            .bind(now)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;
            sqlx::query(&format!(
                "INSERT INTO {schema}.commerce_collection_products (collection_id, product_id, position) VALUES ($1, $2, 0) ON CONFLICT (collection_id, product_id) DO NOTHING",
                schema = self.schema,
            ))
            .bind(&collection_id)
            .bind(&product_id)
            .execute(&mut *transaction)
            .await
            .map_err(database_error)?;

            for site in sites
                .iter()
                .filter(|site| product.site_ids.is_empty() || product.site_ids.contains(&site.id))
            {
                for locale in &site.supported_locales {
                    sqlx::query(&format!(
                        "INSERT INTO {schema}.commerce_product_publications (product_id, site_id, locale, summary, is_published, updated_at) VALUES ($1, $2, $3, $4, TRUE, $5) ON CONFLICT (product_id, site_id, locale) DO NOTHING",
                        schema = self.schema,
                    ))
                    .bind(&product_id)
                    .bind(&site.id)
                    .bind(locale)
                    .bind(&product.summary)
                    .bind(now)
                    .execute(&mut *transaction)
                    .await
                    .map_err(database_error)?;
                }
            }

            for location in &product.inventory_locations {
                sqlx::query(&format!(
                    "INSERT INTO {schema}.commerce_inventory_locations (product_id, location_id, is_available, updated_at) VALUES ($1, $2, TRUE, $3) ON CONFLICT (product_id, location_id) DO NOTHING",
                    schema = self.schema,
                ))
                .bind(&product_id)
                .bind(location)
                .bind(now)
                .execute(&mut *transaction)
                .await
                .map_err(database_error)?;
            }
        }

        transaction.commit().await.map_err(database_error)?;
        Ok(())
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn database_error(error: impl std::fmt::Display) -> ShopprJobError {
    ShopprJobError::unavailable(error.to_string())
}

fn unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}
