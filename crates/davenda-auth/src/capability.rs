use super::*;

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

pub trait AuthModelPackage {
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

pub fn default_auth_model_package() -> DefaultAuthModelPackage {
    DefaultAuthModelPackage::default()
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
