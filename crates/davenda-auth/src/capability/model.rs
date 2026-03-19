use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use zanzibar::Schema;

use crate::{DavendaAuthError, Entity, Namespace, Relation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    SystemModuleManage,
    SystemConfigRead,
    SystemConfigWrite,
    AdminShellAccess,
    AdminAuditRead,
    CmsPageRead,
    CmsPagePublish,
    CmsPageEdit,
    CmsNavigationEdit,
    CatalogProductRead,
    CatalogProductEdit,
    CatalogCollectionEdit,
    CheckoutSessionCreate,
    OrderRead,
    OrderRefundIssue,
    MembershipSubscriptionManage,
    MembershipTierEdit,
    EventsEventPublish,
    EventsSlotManage,
    EventsBookingCreate,
    EventsBookingCheckIn,
    AssetRead,
    AssetReadPublic,
    AssetPublish,
    AssetReplace,
    AssetManageStorage,
    SeoMetadataEdit,
    I18nTranslationEdit,
}

impl Capability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SystemModuleManage => "system.module.manage",
            Self::SystemConfigRead => "system.config.read",
            Self::SystemConfigWrite => "system.config.write",
            Self::AdminShellAccess => "admin.shell.access",
            Self::AdminAuditRead => "admin.audit.read",
            Self::CmsPageRead => "cms.page.read",
            Self::CmsPagePublish => "cms.page.publish",
            Self::CmsPageEdit => "cms.page.edit",
            Self::CmsNavigationEdit => "cms.navigation.edit",
            Self::CatalogProductRead => "catalog.product.read",
            Self::CatalogProductEdit => "catalog.product.edit",
            Self::CatalogCollectionEdit => "catalog.collection.edit",
            Self::CheckoutSessionCreate => "checkout.session.create",
            Self::OrderRead => "order.read",
            Self::OrderRefundIssue => "order.refund.issue",
            Self::MembershipSubscriptionManage => "membership.subscription.manage",
            Self::MembershipTierEdit => "membership.tier.edit",
            Self::EventsEventPublish => "events.event.publish",
            Self::EventsSlotManage => "events.slot.manage",
            Self::EventsBookingCreate => "events.booking.create",
            Self::EventsBookingCheckIn => "events.booking.check_in",
            Self::AssetRead => "asset.read",
            Self::AssetReadPublic => "asset.read_public",
            Self::AssetPublish => "asset.publish",
            Self::AssetReplace => "asset.replace",
            Self::AssetManageStorage => "asset.manage_storage",
            Self::SeoMetadataEdit => "seo.metadata.edit",
            Self::I18nTranslationEdit => "i18n.translation.edit",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageMode {
    Replace,
    Extend,
}

impl fmt::Display for PackageMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replace => f.write_str("replace"),
            Self::Extend => f.write_str("extend"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl PackageVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthModelManifest {
    pub name: String,
    pub version: PackageVersion,
    pub mode: PackageMode,
    pub storage_schema_version: u32,
    pub model_version: u32,
    pub capability_binding_version: u32,
    pub imports: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityBinding {
    pub capability: Capability,
    pub resource_namespaces: Vec<Namespace>,
    pub relation: Relation,
}

impl CapabilityBinding {
    pub fn matches_namespace(&self, namespace: Namespace) -> bool {
        self.resource_namespaces.contains(&namespace)
    }
}

pub trait AuthModelPackage: Send + Sync {
    fn manifest(&self) -> &AuthModelManifest;
    fn schema(&self) -> &Schema;
    fn capability_bindings(&self) -> &HashMap<Capability, CapabilityBinding>;

    fn binding_for(&self, capability: Capability) -> Option<&CapabilityBinding> {
        self.capability_bindings().get(&capability)
    }

    fn resolve_binding(
        &self,
        capability: Capability,
        resource: &Entity,
    ) -> Result<&CapabilityBinding, DavendaAuthError> {
        let binding = self
            .binding_for(capability)
            .ok_or(DavendaAuthError::MissingCapabilityBinding { capability })?;

        if binding.matches_namespace(resource.namespace()) {
            Ok(binding)
        } else {
            Err(DavendaAuthError::ResourceNamespaceMismatch {
                capability,
                actual: resource.namespace(),
                expected: binding.resource_namespaces.clone(),
            })
        }
    }
}

#[derive(Clone)]
pub struct AuthModelPackageSelection {
    package: Arc<dyn AuthModelPackage>,
}

impl AuthModelPackageSelection {
    pub fn new<P>(package: P) -> Self
    where
        P: AuthModelPackage + 'static,
    {
        Self {
            package: Arc::new(package),
        }
    }

    pub fn manifest(&self) -> &AuthModelManifest {
        self.package.manifest()
    }

    pub fn package(&self) -> &dyn AuthModelPackage {
        self.package.as_ref()
    }
}

impl fmt::Debug for AuthModelPackageSelection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthModelPackageSelection")
            .field("manifest", &self.package.manifest())
            .finish()
    }
}
