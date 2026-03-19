use crate::CommerceModelError;
use davenda_auth::Capability;
use davenda_core::{AdminContributionKind, AdminNavigationSection, AdminResourceContribution};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

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
        plan.insert(MigrationStep::new(
            MigrationId::new("001_catalog_products")?,
            owner.clone(),
            10,
            "create catalog products and variants tables",
        )?)?;
        plan.insert(MigrationStep::new(
            MigrationId::new("002_collections")?,
            owner.clone(),
            20,
            "create collections and product membership tables",
        )?)?;
        plan.insert(MigrationStep::new(
            MigrationId::new("003_checkouts_orders")?,
            owner.clone(),
            30,
            "create checkout, order, and pricing snapshot tables",
        )?)?;
        plan.insert(MigrationStep::new(
            MigrationId::new("004_refunds")?,
            owner,
            40,
            "create refund ledger and payment reconciliation tables",
        )?)?;
        Ok(plan)
    }
}

impl Default for CommerceModule {
    fn default() -> Self {
        Self::new()
    }
}
