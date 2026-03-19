use std::collections::HashMap;

use zanzibar::Schema;

use crate::{Namespace, Relation, default_schema};

use super::*;

#[derive(Debug, Clone)]
pub struct DefaultAuthModelPackage {
    manifest: AuthModelManifest,
    schema: Schema,
    capability_bindings: HashMap<Capability, CapabilityBinding>,
}

impl Default for DefaultAuthModelPackage {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultAuthModelPackage {
    pub fn new() -> Self {
        Self {
            manifest: default_manifest(),
            schema: default_schema(),
            capability_bindings: default_capability_bindings(),
        }
    }
}

impl AuthModelPackage for DefaultAuthModelPackage {
    fn manifest(&self) -> &AuthModelManifest {
        &self.manifest
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn capability_bindings(&self) -> &HashMap<Capability, CapabilityBinding> {
        &self.capability_bindings
    }
}

#[derive(Debug, Clone)]
pub struct ConfiguredAuthModelPackage {
    manifest: AuthModelManifest,
    schema: Schema,
    capability_bindings: HashMap<Capability, CapabilityBinding>,
}

impl ConfiguredAuthModelPackage {
    /// Build a package selection that keeps the configured identity but reuses the
    /// shipped default schema and capability bindings.
    pub fn new(name: impl Into<String>) -> Self {
        let mut manifest = default_manifest();
        manifest.name = name.into();
        Self {
            manifest,
            schema: default_schema(),
            capability_bindings: default_capability_bindings(),
        }
    }
}

impl AuthModelPackage for ConfiguredAuthModelPackage {
    fn manifest(&self) -> &AuthModelManifest {
        &self.manifest
    }

    fn schema(&self) -> &Schema {
        &self.schema
    }

    fn capability_bindings(&self) -> &HashMap<Capability, CapabilityBinding> {
        &self.capability_bindings
    }
}

pub fn default_auth_model_package() -> DefaultAuthModelPackage {
    DefaultAuthModelPackage::default()
}

pub fn configured_auth_model_package(name: impl Into<String>) -> ConfiguredAuthModelPackage {
    ConfiguredAuthModelPackage::new(name)
}

/// Build an auth package selection from the deployment-configured package identity.
///
/// This preserves the configured manifest name while reusing the shipped default
/// schema and capability bindings so replacement packages remain live-explainable.
pub fn configured_auth_model_package_selection(
    name: impl Into<String>,
) -> AuthModelPackageSelection {
    AuthModelPackageSelection::new(configured_auth_model_package(name))
}

/// Compatibility alias for callers that already refer to the deployment-selected
/// package identity.
pub fn deployment_auth_model_package(name: impl Into<String>) -> ConfiguredAuthModelPackage {
    ConfiguredAuthModelPackage::new(name)
}

/// Compatibility alias for callers that already refer to the deployment-selected
/// package identity.
pub fn deployment_auth_model_package_selection(
    name: impl Into<String>,
) -> AuthModelPackageSelection {
    configured_auth_model_package_selection(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AuthModelPackage, Capability};

    #[test]
    fn configured_package_selection_preserves_identity_and_bindings() {
        let package = configured_auth_model_package_selection("platform-extended-auth");

        assert_eq!(package.manifest().name, "platform-extended-auth");
        assert_eq!(
            package.package().binding_for(Capability::CmsPageRead).unwrap(),
            DefaultAuthModelPackage::default()
                .binding_for(Capability::CmsPageRead)
                .unwrap()
        );
    }

    #[test]
    fn deployment_package_preserves_identity_while_reusing_default_bindings() {
        let package = deployment_auth_model_package("platform-extended-auth");

        assert_eq!(package.manifest().name, "platform-extended-auth");
        assert_eq!(
            package.binding_for(Capability::CmsPageRead).unwrap(),
            DefaultAuthModelPackage::default()
                .binding_for(Capability::CmsPageRead)
                .unwrap()
        );
    }
}

pub fn default_manifest() -> AuthModelManifest {
    AuthModelManifest {
        name: "platform-default-auth".to_string(),
        version: PackageVersion::new(1, 0, 0),
        mode: PackageMode::Replace,
        storage_schema_version: 1,
        model_version: 1,
        capability_binding_version: 1,
        imports: Vec::new(),
    }
}

pub fn default_capability_bindings() -> HashMap<Capability, CapabilityBinding> {
    HashMap::from([
        binding(
            Capability::SystemModuleManage,
            vec![Namespace::Tenant],
            Relation::Manage,
        ),
        binding(
            Capability::SystemConfigRead,
            vec![Namespace::Tenant],
            Relation::View,
        ),
        binding(
            Capability::SystemConfigWrite,
            vec![Namespace::Tenant],
            Relation::Manage,
        ),
        binding(
            Capability::AdminShellAccess,
            vec![Namespace::AdminModule],
            Relation::View,
        ),
        binding(
            Capability::AdminAuditRead,
            vec![Namespace::AdminModule],
            Relation::Read,
        ),
        binding(
            Capability::CmsPageRead,
            vec![Namespace::Page],
            Relation::View,
        ),
        binding(
            Capability::CmsPagePublish,
            vec![Namespace::Page],
            Relation::Publish,
        ),
        binding(
            Capability::CmsPageEdit,
            vec![Namespace::Page],
            Relation::Edit,
        ),
        binding(
            Capability::CmsNavigationEdit,
            vec![Namespace::Navigation],
            Relation::Edit,
        ),
        binding(
            Capability::CatalogProductRead,
            vec![Namespace::Product],
            Relation::View,
        ),
        binding(
            Capability::CatalogProductEdit,
            vec![Namespace::Product],
            Relation::Edit,
        ),
        binding(
            Capability::CatalogCollectionEdit,
            vec![Namespace::Collection],
            Relation::Edit,
        ),
        binding(
            Capability::CheckoutSessionCreate,
            vec![Namespace::Storefront],
            Relation::Checkout,
        ),
        binding(
            Capability::OrderRead,
            vec![Namespace::Order],
            Relation::View,
        ),
        binding(
            Capability::OrderRefundIssue,
            vec![Namespace::Order],
            Relation::Refund,
        ),
        binding(
            Capability::MembershipSubscriptionManage,
            vec![Namespace::Subscription],
            Relation::Manage,
        ),
        binding(
            Capability::MembershipTierEdit,
            vec![Namespace::MembershipTier],
            Relation::Edit,
        ),
        binding(
            Capability::EventsEventPublish,
            vec![Namespace::Event],
            Relation::Publish,
        ),
        binding(
            Capability::EventsSlotManage,
            vec![Namespace::EventSlot],
            Relation::Manage,
        ),
        binding(
            Capability::EventsBookingCreate,
            vec![Namespace::EventSlot],
            Relation::Book,
        ),
        binding(
            Capability::EventsBookingCheckIn,
            vec![Namespace::Booking],
            Relation::CheckIn,
        ),
        binding(
            Capability::AssetRead,
            vec![Namespace::Asset],
            Relation::Read,
        ),
        binding(
            Capability::AssetReadPublic,
            vec![Namespace::Asset],
            Relation::ReadPublic,
        ),
        binding(
            Capability::AssetPublish,
            vec![Namespace::Asset],
            Relation::Publish,
        ),
        binding(
            Capability::AssetReplace,
            vec![Namespace::Asset],
            Relation::Replace,
        ),
        binding(
            Capability::AssetManageStorage,
            vec![Namespace::Asset],
            Relation::ManageStorage,
        ),
        binding(
            Capability::SeoMetadataEdit,
            vec![Namespace::Page, Namespace::Product, Namespace::Event],
            Relation::Edit,
        ),
        binding(
            Capability::I18nTranslationEdit,
            vec![
                Namespace::Page,
                Namespace::Navigation,
                Namespace::Product,
                Namespace::MembershipTier,
                Namespace::Event,
            ],
            Relation::Edit,
        ),
    ])
}

fn binding(
    capability: Capability,
    resource_namespaces: Vec<Namespace>,
    relation: Relation,
) -> (Capability, CapabilityBinding) {
    (
        capability,
        CapabilityBinding {
            capability,
            resource_namespaces,
            relation,
        },
    )
}
