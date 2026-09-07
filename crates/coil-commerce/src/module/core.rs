use crate::CommerceModelError;
use coil_auth::Capability;
use coil_core::{AdminContributionKind, AdminNavigationSection, AdminResourceContribution};
use coil_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

pub struct CommerceModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceContribution>,
}

impl CommerceModule {
    pub fn new() -> Self {
        Self {
            name: "commerce".to_string(),
            config_namespace: "commerce".to_string(),
            admin_resources: vec![
                AdminResourceContribution::new(
                    "commerce.catalog",
                    "/admin/commerce/catalog",
                    "Catalog",
                    "Catalog",
                    AdminNavigationSection::Commerce,
                    AdminContributionKind::ResourceIndex,
                    Capability::CatalogProductRead,
                ),
                AdminResourceContribution::new(
                    "commerce.collections",
                    "/admin/commerce/collections",
                    "Collections",
                    "Collections",
                    AdminNavigationSection::Commerce,
                    AdminContributionKind::ResourceIndex,
                    Capability::CatalogCollectionEdit,
                ),
                AdminResourceContribution::new(
                    "commerce.checkouts",
                    "/admin/commerce/checkouts",
                    "Checkouts",
                    "Checkouts",
                    AdminNavigationSection::Commerce,
                    AdminContributionKind::ResourceIndex,
                    Capability::CheckoutSessionCreate,
                ),
                AdminResourceContribution::new(
                    "commerce.orders",
                    "/admin/commerce/orders",
                    "Orders",
                    "Orders",
                    AdminNavigationSection::Commerce,
                    AdminContributionKind::ResourceIndex,
                    Capability::OrderRead,
                ),
            ],
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config_namespace(&self) -> &str {
        &self.config_namespace
    }

    pub fn admin_resources(&self) -> &[AdminResourceContribution] {
        &self.admin_resources
    }

    pub fn migration_plan(&self) -> Result<MigrationPlan, CommerceModelError> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("001_catalog_products")?,
                owner.clone(),
                10,
                "create catalog products and variants tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_catalog_products (id TEXT PRIMARY KEY, slug TEXT NOT NULL, sku TEXT NOT NULL, title TEXT NOT NULL, product_type TEXT NOT NULL, status TEXT NOT NULL, price_minor BIGINT NOT NULL, currency TEXT NOT NULL, source_system TEXT, source_key TEXT UNIQUE, import_batch_id TEXT, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_catalog_variants (id TEXT PRIMARY KEY, product_id TEXT NOT NULL, sku TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, price_minor BIGINT NOT NULL, currency TEXT NOT NULL, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("002_collections")?,
                owner.clone(),
                20,
                "create collections and product membership tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_collections (id TEXT PRIMARY KEY, handle TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_collection_products (collection_id TEXT NOT NULL, product_id TEXT NOT NULL, position BIGINT NOT NULL, PRIMARY KEY (collection_id, product_id))",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("003_checkouts_orders")?,
                owner.clone(),
                30,
                "create checkout, order, and pricing snapshot tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_checkouts (id TEXT PRIMARY KEY, status TEXT NOT NULL, currency TEXT NOT NULL, email TEXT, principal_id TEXT, subtotal_minor BIGINT NOT NULL, total_minor BIGINT NOT NULL, payment_reference TEXT, source_system TEXT, source_key TEXT UNIQUE, import_batch_id TEXT, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_orders (id TEXT PRIMARY KEY, checkout_id TEXT, status TEXT NOT NULL, currency TEXT NOT NULL, email TEXT, principal_id TEXT, subtotal_minor BIGINT NOT NULL, total_minor BIGINT NOT NULL, payment_status TEXT NOT NULL, payment_reference TEXT, source_system TEXT, source_key TEXT UNIQUE, import_batch_id TEXT, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_order_lines (id TEXT PRIMARY KEY, order_id TEXT NOT NULL, product_id TEXT, variant_id TEXT, title TEXT NOT NULL, quantity BIGINT NOT NULL, line_total_minor BIGINT NOT NULL, currency TEXT NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("004_refunds")?,
                owner.clone(),
                40,
                "create refund ledger and payment reconciliation tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_refunds (id TEXT PRIMARY KEY, order_id TEXT NOT NULL, status TEXT NOT NULL, amount_minor BIGINT NOT NULL, currency TEXT NOT NULL, reason TEXT NOT NULL, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_payment_reconciliation (id TEXT PRIMARY KEY, order_id TEXT NOT NULL, provider TEXT NOT NULL, provider_reference TEXT NOT NULL, status TEXT NOT NULL, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("005_catalog_publication")?,
                owner.clone(),
                50,
                "create site-scoped catalogue publication and inventory tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_product_publications (product_id TEXT NOT NULL REFERENCES commerce_catalog_products(id), site_id TEXT NOT NULL, locale TEXT NOT NULL, summary TEXT NOT NULL, is_published BOOLEAN NOT NULL DEFAULT FALSE, updated_at BIGINT NOT NULL, PRIMARY KEY (product_id, site_id, locale))",
            )?
            .with_statement(
                "CREATE INDEX IF NOT EXISTS commerce_product_publications_lookup ON commerce_product_publications (site_id, locale, is_published, product_id)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_collection_publications (collection_id TEXT NOT NULL REFERENCES commerce_collections(id), site_id TEXT NOT NULL, locale TEXT NOT NULL, label TEXT NOT NULL, summary TEXT NOT NULL, is_published BOOLEAN NOT NULL DEFAULT FALSE, updated_at BIGINT NOT NULL, PRIMARY KEY (collection_id, site_id, locale))",
            )?
            .with_statement(
                "CREATE INDEX IF NOT EXISTS commerce_collection_publications_lookup ON commerce_collection_publications (site_id, locale, is_published, collection_id)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_inventory_locations (product_id TEXT NOT NULL REFERENCES commerce_catalog_products(id), location_id TEXT NOT NULL, is_available BOOLEAN NOT NULL DEFAULT TRUE, updated_at BIGINT NOT NULL, PRIMARY KEY (product_id, location_id))",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("006_session_carts")?,
                owner,
                60,
                "create site-scoped session carts and their authoritative lines",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_carts (site_id TEXT NOT NULL, session_id TEXT NOT NULL, principal_id TEXT, status TEXT NOT NULL, currency TEXT NOT NULL, updated_at BIGINT NOT NULL, PRIMARY KEY (site_id, session_id))",
            )?
            .with_statement(
                "CREATE INDEX IF NOT EXISTS commerce_carts_by_principal ON commerce_carts (site_id, principal_id, updated_at DESC) WHERE principal_id IS NOT NULL",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS commerce_cart_lines (site_id TEXT NOT NULL, session_id TEXT NOT NULL, product_id TEXT NOT NULL REFERENCES commerce_catalog_products(id), title TEXT NOT NULL, quantity BIGINT NOT NULL CHECK (quantity > 0), unit_price_minor BIGINT NOT NULL, currency TEXT NOT NULL, updated_at BIGINT NOT NULL, PRIMARY KEY (site_id, session_id, product_id), FOREIGN KEY (site_id, session_id) REFERENCES commerce_carts(site_id, session_id) ON DELETE CASCADE)",
            )?,
        )?;
        Ok(plan)
    }
}

impl Default for CommerceModule {
    fn default() -> Self {
        Self::new()
    }
}
