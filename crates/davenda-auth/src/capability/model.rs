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
    EventsBookingManage,
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
            Self::EventsBookingManage => "events.booking.manage",
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

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "system.module.manage" => Some(Self::SystemModuleManage),
            "system.config.read" => Some(Self::SystemConfigRead),
            "system.config.write" => Some(Self::SystemConfigWrite),
            "admin.shell.access" => Some(Self::AdminShellAccess),
            "admin.audit.read" => Some(Self::AdminAuditRead),
            "cms.page.read" => Some(Self::CmsPageRead),
            "cms.page.publish" => Some(Self::CmsPagePublish),
            "cms.page.edit" => Some(Self::CmsPageEdit),
            "cms.navigation.edit" => Some(Self::CmsNavigationEdit),
            "catalog.product.read" => Some(Self::CatalogProductRead),
            "catalog.product.edit" => Some(Self::CatalogProductEdit),
            "catalog.collection.edit" => Some(Self::CatalogCollectionEdit),
            "checkout.session.create" => Some(Self::CheckoutSessionCreate),
            "order.read" => Some(Self::OrderRead),
            "order.refund.issue" => Some(Self::OrderRefundIssue),
            "membership.subscription.manage" => Some(Self::MembershipSubscriptionManage),
            "membership.tier.edit" => Some(Self::MembershipTierEdit),
            "events.event.publish" => Some(Self::EventsEventPublish),
            "events.slot.manage" => Some(Self::EventsSlotManage),
            "events.booking.manage" => Some(Self::EventsBookingManage),
            "events.booking.create" => Some(Self::EventsBookingCreate),
            "events.booking.check_in" => Some(Self::EventsBookingCheckIn),
            "asset.read" => Some(Self::AssetRead),
            "asset.read_public" => Some(Self::AssetReadPublic),
            "asset.publish" => Some(Self::AssetPublish),
            "asset.replace" => Some(Self::AssetReplace),
            "asset.manage_storage" => Some(Self::AssetManageStorage),
            "seo.metadata.edit" => Some(Self::SeoMetadataEdit),
            "i18n.translation.edit" => Some(Self::I18nTranslationEdit),
            _ => None,
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
