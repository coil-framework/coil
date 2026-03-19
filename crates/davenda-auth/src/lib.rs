use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use zanzibar::{
    CheckRequest, NamespaceConfig, Object, RebacEngine, RebacError, RelationRule, Schema,
    SchemaBuilder, Subject, Tuple, TupleUpdate,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Namespace {
    Tenant,
    Site,
    Brand,
    Storefront,
    User,
    Group,
    Team,
    ServiceAccount,
    Page,
    Navigation,
    Product,
    Collection,
    Order,
    Subscription,
    MembershipTier,
    Event,
    EventSlot,
    Booking,
    Media,
    MediaLibrary,
    Asset,
    AssetFolder,
    ThemeAssetBundle,
    AdminModule,
}

impl Namespace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tenant => "tenant",
            Self::Site => "site",
            Self::Brand => "brand",
            Self::Storefront => "storefront",
            Self::User => "user",
            Self::Group => "group",
            Self::Team => "team",
            Self::ServiceAccount => "service_account",
            Self::Page => "page",
            Self::Navigation => "navigation",
            Self::Product => "product",
            Self::Collection => "collection",
            Self::Order => "order",
            Self::Subscription => "subscription",
            Self::MembershipTier => "membership_tier",
            Self::Event => "event",
            Self::EventSlot => "event_slot",
            Self::Booking => "booking",
            Self::Media => "media",
            Self::MediaLibrary => "media_library",
            Self::Asset => "asset",
            Self::AssetFolder => "asset_folder",
            Self::ThemeAssetBundle => "theme_asset_bundle",
            Self::AdminModule => "admin_module",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "tenant" => Some(Self::Tenant),
            "site" => Some(Self::Site),
            "brand" => Some(Self::Brand),
            "storefront" => Some(Self::Storefront),
            "user" => Some(Self::User),
            "group" => Some(Self::Group),
            "team" => Some(Self::Team),
            "service_account" => Some(Self::ServiceAccount),
            "page" => Some(Self::Page),
            "navigation" => Some(Self::Navigation),
            "product" => Some(Self::Product),
            "collection" => Some(Self::Collection),
            "order" => Some(Self::Order),
            "subscription" => Some(Self::Subscription),
            "membership_tier" => Some(Self::MembershipTier),
            "event" => Some(Self::Event),
            "event_slot" => Some(Self::EventSlot),
            "booking" => Some(Self::Booking),
            "media" => Some(Self::Media),
            "media_library" => Some(Self::MediaLibrary),
            "asset" => Some(Self::Asset),
            "asset_folder" => Some(Self::AssetFolder),
            "theme_asset_bundle" => Some(Self::ThemeAssetBundle),
            "admin_module" => Some(Self::AdminModule),
            _ => None,
        }
    }
}

impl fmt::Display for Namespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relation {
    Tenant,
    Site,
    Brand,
    Storefront,
    Event,
    Slot,
    Folder,
    Library,
    Member,
    Owner,
    Admin,
    Editor,
    Viewer,
    Support,
    View,
    Edit,
    Publish,
    Manage,
    Checkout,
    Refund,
    Read,
    ReadPublic,
    Replace,
    Delete,
    Unpublish,
    ManageStorage,
    Book,
    CheckIn,
}

impl Relation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tenant => "tenant",
            Self::Site => "site",
            Self::Brand => "brand",
            Self::Storefront => "storefront",
            Self::Event => "event",
            Self::Slot => "slot",
            Self::Folder => "folder",
            Self::Library => "library",
            Self::Member => "member",
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
            Self::Support => "support",
            Self::View => "view",
            Self::Edit => "edit",
            Self::Publish => "publish",
            Self::Manage => "manage",
            Self::Checkout => "checkout",
            Self::Refund => "refund",
            Self::Read => "read",
            Self::ReadPublic => "read_public",
            Self::Replace => "replace",
            Self::Delete => "delete",
            Self::Unpublish => "unpublish",
            Self::ManageStorage => "manage_storage",
            Self::Book => "book",
            Self::CheckIn => "check_in",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "tenant" => Some(Self::Tenant),
            "site" => Some(Self::Site),
            "brand" => Some(Self::Brand),
            "storefront" => Some(Self::Storefront),
            "event" => Some(Self::Event),
            "slot" => Some(Self::Slot),
            "folder" => Some(Self::Folder),
            "library" => Some(Self::Library),
            "member" => Some(Self::Member),
            "owner" => Some(Self::Owner),
            "admin" => Some(Self::Admin),
            "editor" => Some(Self::Editor),
            "viewer" => Some(Self::Viewer),
            "support" => Some(Self::Support),
            "view" => Some(Self::View),
            "edit" => Some(Self::Edit),
            "publish" => Some(Self::Publish),
            "manage" => Some(Self::Manage),
            "checkout" => Some(Self::Checkout),
            "refund" => Some(Self::Refund),
            "read" => Some(Self::Read),
            "read_public" => Some(Self::ReadPublic),
            "replace" => Some(Self::Replace),
            "delete" => Some(Self::Delete),
            "unpublish" => Some(Self::Unpublish),
            "manage_storage" => Some(Self::ManageStorage),
            "book" => Some(Self::Book),
            "check_in" => Some(Self::CheckIn),
            _ => None,
        }
    }
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Entity {
    Tenant(String),
    Site(String),
    Brand(String),
    Storefront(String),
    User(String),
    Group(String),
    Team(String),
    ServiceAccount(String),
    Page(String),
    Navigation(String),
    Product(String),
    Collection(String),
    Order(String),
    Subscription(String),
    MembershipTier(String),
    Event(String),
    EventSlot(String),
    Booking(String),
    Media(String),
    MediaLibrary(String),
    Asset(String),
    AssetFolder(String),
    ThemeAssetBundle(String),
    AdminModule(String),
}

impl Entity {
    pub fn tenant(id: impl Into<String>) -> Self {
        Self::Tenant(id.into())
    }

    pub fn site(id: impl Into<String>) -> Self {
        Self::Site(id.into())
    }

    pub fn brand(id: impl Into<String>) -> Self {
        Self::Brand(id.into())
    }

    pub fn storefront(id: impl Into<String>) -> Self {
        Self::Storefront(id.into())
    }

    pub fn user(id: impl Into<String>) -> Self {
        Self::User(id.into())
    }

    pub fn any_user() -> Self {
        Self::User("*".into())
    }

    pub fn group(id: impl Into<String>) -> Self {
        Self::Group(id.into())
    }

    pub fn team(id: impl Into<String>) -> Self {
        Self::Team(id.into())
    }

    pub fn service_account(id: impl Into<String>) -> Self {
        Self::ServiceAccount(id.into())
    }

    pub fn page(id: impl Into<String>) -> Self {
        Self::Page(id.into())
    }

    pub fn navigation(id: impl Into<String>) -> Self {
        Self::Navigation(id.into())
    }

    pub fn product(id: impl Into<String>) -> Self {
        Self::Product(id.into())
    }

    pub fn collection(id: impl Into<String>) -> Self {
        Self::Collection(id.into())
    }

    pub fn order(id: impl Into<String>) -> Self {
        Self::Order(id.into())
    }

    pub fn subscription(id: impl Into<String>) -> Self {
        Self::Subscription(id.into())
    }

    pub fn membership_tier(id: impl Into<String>) -> Self {
        Self::MembershipTier(id.into())
    }

    pub fn event(id: impl Into<String>) -> Self {
        Self::Event(id.into())
    }

    pub fn event_slot(id: impl Into<String>) -> Self {
        Self::EventSlot(id.into())
    }

    pub fn booking(id: impl Into<String>) -> Self {
        Self::Booking(id.into())
    }

    pub fn media(id: impl Into<String>) -> Self {
        Self::Media(id.into())
    }

    pub fn media_library(id: impl Into<String>) -> Self {
        Self::MediaLibrary(id.into())
    }

    pub fn asset(id: impl Into<String>) -> Self {
        Self::Asset(id.into())
    }

    pub fn asset_folder(id: impl Into<String>) -> Self {
        Self::AssetFolder(id.into())
    }

    pub fn theme_asset_bundle(id: impl Into<String>) -> Self {
        Self::ThemeAssetBundle(id.into())
    }

    pub fn admin_module(id: impl Into<String>) -> Self {
        Self::AdminModule(id.into())
    }

    pub const fn namespace(&self) -> Namespace {
        match self {
            Self::Tenant(_) => Namespace::Tenant,
            Self::Site(_) => Namespace::Site,
            Self::Brand(_) => Namespace::Brand,
            Self::Storefront(_) => Namespace::Storefront,
            Self::User(_) => Namespace::User,
            Self::Group(_) => Namespace::Group,
            Self::Team(_) => Namespace::Team,
            Self::ServiceAccount(_) => Namespace::ServiceAccount,
            Self::Page(_) => Namespace::Page,
            Self::Navigation(_) => Namespace::Navigation,
            Self::Product(_) => Namespace::Product,
            Self::Collection(_) => Namespace::Collection,
            Self::Order(_) => Namespace::Order,
            Self::Subscription(_) => Namespace::Subscription,
            Self::MembershipTier(_) => Namespace::MembershipTier,
            Self::Event(_) => Namespace::Event,
            Self::EventSlot(_) => Namespace::EventSlot,
            Self::Booking(_) => Namespace::Booking,
            Self::Media(_) => Namespace::Media,
            Self::MediaLibrary(_) => Namespace::MediaLibrary,
            Self::Asset(_) => Namespace::Asset,
            Self::AssetFolder(_) => Namespace::AssetFolder,
            Self::ThemeAssetBundle(_) => Namespace::ThemeAssetBundle,
            Self::AdminModule(_) => Namespace::AdminModule,
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Tenant(id)
            | Self::Site(id)
            | Self::Brand(id)
            | Self::Storefront(id)
            | Self::User(id)
            | Self::Group(id)
            | Self::Team(id)
            | Self::ServiceAccount(id)
            | Self::Page(id)
            | Self::Navigation(id)
            | Self::Product(id)
            | Self::Collection(id)
            | Self::Order(id)
            | Self::Subscription(id)
            | Self::MembershipTier(id)
            | Self::Event(id)
            | Self::EventSlot(id)
            | Self::Booking(id)
            | Self::Media(id)
            | Self::MediaLibrary(id)
            | Self::Asset(id)
            | Self::AssetFolder(id)
            | Self::ThemeAssetBundle(id)
            | Self::AdminModule(id) => id,
        }
    }

    pub fn to_object(&self) -> Object {
        Object {
            namespace: self.namespace().to_string(),
            id: self.id().to_owned(),
        }
    }

    pub fn from_object(object: &Object) -> Option<Self> {
        let id = object.id.clone();

        match Namespace::from_str(&object.namespace)? {
            Namespace::Tenant => Some(Self::Tenant(id)),
            Namespace::Site => Some(Self::Site(id)),
            Namespace::Brand => Some(Self::Brand(id)),
            Namespace::Storefront => Some(Self::Storefront(id)),
            Namespace::User => Some(Self::User(id)),
            Namespace::Group => Some(Self::Group(id)),
            Namespace::Team => Some(Self::Team(id)),
            Namespace::ServiceAccount => Some(Self::ServiceAccount(id)),
            Namespace::Page => Some(Self::Page(id)),
            Namespace::Navigation => Some(Self::Navigation(id)),
            Namespace::Product => Some(Self::Product(id)),
            Namespace::Collection => Some(Self::Collection(id)),
            Namespace::Order => Some(Self::Order(id)),
            Namespace::Subscription => Some(Self::Subscription(id)),
            Namespace::MembershipTier => Some(Self::MembershipTier(id)),
            Namespace::Event => Some(Self::Event(id)),
            Namespace::EventSlot => Some(Self::EventSlot(id)),
            Namespace::Booking => Some(Self::Booking(id)),
            Namespace::Media => Some(Self::Media(id)),
            Namespace::MediaLibrary => Some(Self::MediaLibrary(id)),
            Namespace::Asset => Some(Self::Asset(id)),
            Namespace::AssetFolder => Some(Self::AssetFolder(id)),
            Namespace::ThemeAssetBundle => Some(Self::ThemeAssetBundle(id)),
            Namespace::AdminModule => Some(Self::AdminModule(id)),
        }
    }

    pub fn as_subject(&self) -> DefaultSubject {
        DefaultSubject::Entity(self.clone())
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace(), self.id())
    }
}

impl From<&Entity> for Object {
    fn from(value: &Entity) -> Self {
        value.to_object()
    }
}

impl From<Entity> for Object {
    fn from(value: Entity) -> Self {
        value.to_object()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefaultSubject {
    Entity(Entity),
    Userset { object: Entity, relation: Relation },
}

impl DefaultSubject {
    pub fn entity(entity: Entity) -> Self {
        Self::Entity(entity)
    }

    pub fn userset(object: Entity, relation: Relation) -> Self {
        Self::Userset { object, relation }
    }

    pub fn to_subject(&self) -> Subject {
        match self {
            Self::Entity(entity) => Subject::Entity(entity.to_object()),
            Self::Userset { object, relation } => Subject::Userset {
                object: object.to_object(),
                relation: relation.to_string(),
            },
        }
    }

    pub fn from_subject(subject: &Subject) -> Option<Self> {
        match subject {
            Subject::Entity(object) => Some(Self::Entity(Entity::from_object(object)?)),
            Subject::Userset { object, relation } => Some(Self::Userset {
                object: Entity::from_object(object)?,
                relation: Relation::from_str(relation)?,
            }),
        }
    }
}

impl From<&DefaultSubject> for Subject {
    fn from(value: &DefaultSubject) -> Self {
        value.to_subject()
    }
}

impl From<DefaultSubject> for Subject {
    fn from(value: DefaultSubject) -> Self {
        value.to_subject()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefaultTuple {
    pub object: Entity,
    pub relation: Relation,
    pub subject: DefaultSubject,
}

impl DefaultTuple {
    pub fn new(object: Entity, relation: Relation, subject: DefaultSubject) -> Self {
        Self {
            object,
            relation,
            subject,
        }
    }

    pub fn to_tuple(&self) -> Tuple {
        Tuple {
            object: self.object.to_object(),
            relation: self.relation.to_string(),
            subject: self.subject.to_subject(),
        }
    }

    pub fn from_tuple(tuple: &Tuple) -> Option<Self> {
        Some(Self {
            object: Entity::from_object(&tuple.object)?,
            relation: Relation::from_str(&tuple.relation)?,
            subject: DefaultSubject::from_subject(&tuple.subject)?,
        })
    }
}

impl From<&DefaultTuple> for Tuple {
    fn from(value: &DefaultTuple) -> Self {
        value.to_tuple()
    }
}

impl From<DefaultTuple> for Tuple {
    fn from(value: DefaultTuple) -> Self {
        value.to_tuple()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefaultTupleUpdate {
    Write(DefaultTuple),
    Delete(DefaultTuple),
}

impl From<DefaultTupleUpdate> for TupleUpdate {
    fn from(value: DefaultTupleUpdate) -> Self {
        match value {
            DefaultTupleUpdate::Write(tuple) => TupleUpdate::Write(tuple.into()),
            DefaultTupleUpdate::Delete(tuple) => TupleUpdate::Delete(tuple.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessCheck {
    pub subject: DefaultSubject,
    pub relation: Relation,
    pub object: Entity,
}

impl AccessCheck {
    pub fn new(subject: DefaultSubject, relation: Relation, object: Entity) -> Self {
        Self {
            subject,
            relation,
            object,
        }
    }
}

impl From<AccessCheck> for CheckRequest {
    fn from(value: AccessCheck) -> Self {
        Self {
            subject: value.subject.into(),
            relation: value.relation.to_string(),
            object: value.object.into(),
        }
    }
}

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

pub const DEFAULT_EXPLAIN_MAX_DEPTH: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExplainOptions {
    pub max_depth: usize,
    pub cycle_protection: bool,
}

impl ExplainOptions {
    pub const fn new(max_depth: usize) -> Self {
        Self {
            max_depth,
            cycle_protection: true,
        }
    }

    pub const fn with_cycle_protection(mut self, cycle_protection: bool) -> Self {
        self.cycle_protection = cycle_protection;
        self
    }

    pub const fn normalized(self) -> Self {
        let max_depth = if self.max_depth == 0 {
            1
        } else {
            self.max_depth
        };

        Self {
            max_depth,
            cycle_protection: self.cycle_protection,
        }
    }
}

impl Default for ExplainOptions {
    fn default() -> Self {
        Self {
            max_depth: DEFAULT_EXPLAIN_MAX_DEPTH,
            cycle_protection: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplainDecision {
    Allow,
    Deny,
}

impl ExplainDecision {
    pub const fn is_allowed(self) -> bool {
        matches!(self, Self::Allow)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExplainedNode {
    pub object: Entity,
    pub relation: Option<Relation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainStep {
    Start {
        node: ExplainedNode,
    },
    DirectSubjectMatch {
        node: ExplainedNode,
    },
    TupleSubjectMatch {
        from: ExplainedNode,
        tuple: DefaultTuple,
    },
    Inherit {
        from: ExplainedNode,
        to: ExplainedNode,
    },
    TupleTraversal {
        from: ExplainedNode,
        tuple: DefaultTuple,
        to: ExplainedNode,
    },
    Computed {
        from: ExplainedNode,
        via_tuple: DefaultTuple,
        to: ExplainedNode,
    },
    TupleToUserset {
        from: ExplainedNode,
        via_tuple: DefaultTuple,
        to: ExplainedNode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeniedReason {
    NoMatchingPath,
    RecursionLimitReached { max_depth: usize },
    CycleDetected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeniedAttempt {
    Inherit {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
    TupleTraversal {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
    Computed {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
    TupleToUserset {
        step: ExplainStep,
        result: Box<DeniedExplanation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeniedExplanation {
    pub node: ExplainedNode,
    pub reason: DeniedReason,
    pub attempts: Vec<DeniedAttempt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedExplanation {
    pub steps: Vec<ExplainStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplainTrace {
    Allowed(AllowedExplanation),
    Denied(DeniedExplanation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExplanation {
    pub manifest: AuthModelManifest,
    pub subject: DefaultSubject,
    pub capability: Capability,
    pub object: Entity,
    pub binding: CapabilityBinding,
    pub decision: ExplainDecision,
    pub options: ExplainOptions,
    pub trace: ExplainTrace,
}

#[derive(Debug)]
pub enum DavendaAuthError {
    Rebac(RebacError),
    MissingCapabilityBinding {
        capability: Capability,
    },
    ResourceNamespaceMismatch {
        capability: Capability,
        actual: Namespace,
        expected: Vec<Namespace>,
    },
    UnsupportedExplainNamespace {
        namespace: String,
    },
    UnsupportedExplainRelation {
        relation: String,
    },
}

impl fmt::Display for DavendaAuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rebac(error) => write!(f, "{error}"),
            Self::MissingCapabilityBinding { capability } => {
                write!(f, "no binding exists for capability `{capability}`")
            }
            Self::ResourceNamespaceMismatch {
                capability,
                actual,
                expected,
            } => {
                let expected = expected
                    .iter()
                    .map(Namespace::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(
                    f,
                    "capability `{capability}` does not apply to `{actual}` resources; expected one of [{expected}]"
                )
            }
            Self::UnsupportedExplainNamespace { namespace } => {
                write!(
                    f,
                    "cannot explain tuples or schema entries for unsupported namespace `{namespace}`"
                )
            }
            Self::UnsupportedExplainRelation { relation } => {
                write!(
                    f,
                    "cannot explain tuples or schema entries for unsupported relation `{relation}`"
                )
            }
        }
    }
}

impl Error for DavendaAuthError {}

impl From<RebacError> for DavendaAuthError {
    fn from(value: RebacError) -> Self {
        Self::Rebac(value)
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

#[derive(Clone)]
pub struct DavendaAuth<E> {
    engine: E,
    tenant_id: i64,
}

impl<E> DavendaAuth<E> {
    pub fn new(engine: E, tenant_id: i64) -> Self {
        Self { engine, tenant_id }
    }

    pub fn tenant_id(&self) -> i64 {
        self.tenant_id
    }

    pub fn engine(&self) -> &E {
        &self.engine
    }

    pub fn into_inner(self) -> E {
        self.engine
    }
}

impl<E> DavendaAuth<E>
where
    E: RebacEngine,
{
    pub async fn apply_default_schema(&self) -> Result<(), RebacError> {
        self.apply_model_package(&DefaultAuthModelPackage::default())
            .await
    }

    pub async fn apply_model_package<P>(&self, package: &P) -> Result<(), RebacError>
    where
        P: AuthModelPackage,
    {
        self.engine
            .apply_schema(self.tenant_id, package.schema().clone())
            .await
    }

    pub async fn write(
        &self,
        updates: impl IntoIterator<Item = DefaultTupleUpdate>,
    ) -> Result<(), RebacError> {
        self.engine
            .write_tuples(
                self.tenant_id,
                updates.into_iter().map(Into::into).collect(),
            )
            .await
    }

    pub async fn check(
        &self,
        subject: &DefaultSubject,
        relation: Relation,
        object: &Entity,
    ) -> Result<bool, RebacError> {
        let subject = subject.to_subject();
        let object = object.to_object();

        self.engine
            .check(self.tenant_id, &subject, relation.as_str(), &object)
            .await
    }

    pub async fn check_capability<P>(
        &self,
        package: &P,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<bool, DavendaAuthError>
    where
        P: AuthModelPackage,
    {
        let binding = package.resolve_binding(capability, object)?;
        let subject = subject.to_subject();
        let object = object.to_object();

        Ok(self
            .engine
            .check(self.tenant_id, &subject, binding.relation.as_str(), &object)
            .await?)
    }

    pub async fn check_default_capability(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<bool, DavendaAuthError> {
        self.check_capability(
            &DefaultAuthModelPackage::default(),
            subject,
            capability,
            object,
        )
        .await
    }

    pub async fn explain_capability<P>(
        &self,
        package: &P,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<CapabilityExplanation, DavendaAuthError>
    where
        P: AuthModelPackage,
    {
        self.explain_capability_with_options(
            package,
            subject,
            capability,
            object,
            ExplainOptions::default(),
        )
        .await
    }

    pub async fn explain_capability_with_options<P>(
        &self,
        package: &P,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
        options: ExplainOptions,
    ) -> Result<CapabilityExplanation, DavendaAuthError>
    where
        P: AuthModelPackage,
    {
        let tuples = self
            .engine
            .read_tuples(self.tenant_id, None, None, None)
            .await?;

        build_capability_explanation(package, &tuples, subject, capability, object, options)
    }

    pub async fn explain_default_capability(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
    ) -> Result<CapabilityExplanation, DavendaAuthError> {
        self.explain_default_capability_with_options(
            subject,
            capability,
            object,
            ExplainOptions::default(),
        )
        .await
    }

    pub async fn explain_default_capability_with_options(
        &self,
        subject: &DefaultSubject,
        capability: Capability,
        object: &Entity,
        options: ExplainOptions,
    ) -> Result<CapabilityExplanation, DavendaAuthError> {
        self.explain_capability_with_options(
            &DefaultAuthModelPackage::default(),
            subject,
            capability,
            object,
            options,
        )
        .await
    }

    pub async fn check_many(
        &self,
        requests: impl IntoIterator<Item = AccessCheck>,
    ) -> Result<Vec<bool>, RebacError> {
        self.engine
            .check_many(
                self.tenant_id,
                requests.into_iter().map(Into::into).collect(),
            )
            .await
    }

    pub async fn list_objects(
        &self,
        subject: &DefaultSubject,
        relation: Relation,
        namespace: Namespace,
    ) -> Result<Vec<String>, RebacError> {
        let subject = subject.to_subject();

        self.engine
            .list_objects(
                self.tenant_id,
                &subject,
                relation.as_str(),
                namespace.as_str(),
            )
            .await
    }

    pub async fn list_subject_ids(
        &self,
        object: &Entity,
        relation: Relation,
        namespace: Namespace,
    ) -> Result<Vec<String>, RebacError> {
        let object = object.to_object();

        self.engine
            .list_subjects(
                self.tenant_id,
                &object,
                relation.as_str(),
                namespace.as_str(),
            )
            .await
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

pub fn default_schema() -> Schema {
    SchemaBuilder::new()
        .namespace(Namespace::Tenant.as_str(), top_level_namespace())
        .namespace(
            Namespace::Site.as_str(),
            inherited_namespace(Relation::Tenant),
        )
        .namespace(
            Namespace::Brand.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Storefront.as_str(),
            storefront_namespace(Relation::Brand),
        )
        .namespace(Namespace::Group.as_str(), principal_set_namespace())
        .namespace(Namespace::Team.as_str(), principal_set_namespace())
        .namespace(
            Namespace::Page.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Navigation.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Product.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Collection.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Order.as_str(),
            order_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Subscription.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::MembershipTier.as_str(),
            inherited_namespace(Relation::Storefront),
        )
        .namespace(
            Namespace::Event.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::EventSlot.as_str(),
            event_slot_namespace(Relation::Event),
        )
        .namespace(
            Namespace::Booking.as_str(),
            booking_namespace(Relation::Slot),
        )
        .namespace(
            Namespace::MediaLibrary.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::Media.as_str(),
            media_namespace(Relation::Library),
        )
        .namespace(
            Namespace::AssetFolder.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(Namespace::Asset.as_str(), asset_namespace(Relation::Folder))
        .namespace(
            Namespace::ThemeAssetBundle.as_str(),
            inherited_namespace(Relation::Site),
        )
        .namespace(
            Namespace::AdminModule.as_str(),
            admin_module_namespace(Relation::Site),
        )
        .build()
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

fn top_level_namespace() -> NamespaceConfig {
    NamespaceConfig {
        rules: permission_ladder(),
    }
}

fn principal_set_namespace() -> NamespaceConfig {
    let mut rules = permission_ladder();
    rules.insert(
        Relation::View.to_string(),
        vec![
            inherit(Relation::Member),
            inherit(Relation::Viewer),
            inherit(Relation::Support),
            inherit(Relation::Edit),
        ],
    );
    NamespaceConfig { rules }
}

fn inherited_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    NamespaceConfig { rules }
}

fn storefront_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::Checkout.to_string(),
        vec![inherit(Relation::View), inherit(Relation::Member)],
    );
    NamespaceConfig { rules }
}

fn order_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::Refund.to_string(),
        vec![inherit(Relation::Manage), inherit(Relation::Support)],
    );
    NamespaceConfig { rules }
}

fn event_slot_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::Book.to_string(),
        vec![inherit(Relation::View), inherit(Relation::Member)],
    );
    NamespaceConfig { rules }
}

fn booking_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(
        Relation::CheckIn.to_string(),
        vec![inherit(Relation::Manage), inherit(Relation::Support)],
    );
    NamespaceConfig { rules }
}

fn media_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(Relation::Read.to_string(), vec![inherit(Relation::View)]);
    NamespaceConfig { rules }
}

fn admin_module_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(Relation::Read.to_string(), vec![inherit(Relation::View)]);
    NamespaceConfig { rules }
}

fn asset_namespace(link_relation: Relation) -> NamespaceConfig {
    let mut rules = permission_ladder();
    add_inherited_roles(&mut rules, link_relation);
    rules.insert(Relation::Read.to_string(), vec![inherit(Relation::View)]);
    rules.insert(Relation::Replace.to_string(), vec![inherit(Relation::Edit)]);
    rules.insert(
        Relation::Delete.to_string(),
        vec![inherit(Relation::Manage)],
    );
    rules.insert(
        Relation::Unpublish.to_string(),
        vec![inherit(Relation::Manage)],
    );
    rules.insert(
        Relation::ManageStorage.to_string(),
        vec![inherit(Relation::Manage)],
    );
    NamespaceConfig { rules }
}

fn permission_ladder() -> HashMap<String, Vec<RelationRule>> {
    HashMap::from([
        (
            Relation::Manage.to_string(),
            vec![inherit(Relation::Owner), inherit(Relation::Admin)],
        ),
        (
            Relation::Publish.to_string(),
            vec![inherit(Relation::Manage), inherit(Relation::Editor)],
        ),
        (
            Relation::Edit.to_string(),
            vec![inherit(Relation::Publish), inherit(Relation::Editor)],
        ),
        (
            Relation::View.to_string(),
            vec![
                inherit(Relation::Edit),
                inherit(Relation::Viewer),
                inherit(Relation::Support),
            ],
        ),
    ])
}

fn add_inherited_roles(rules: &mut HashMap<String, Vec<RelationRule>>, link_relation: Relation) {
    for relation in [
        Relation::Member,
        Relation::Owner,
        Relation::Admin,
        Relation::Editor,
        Relation::Viewer,
        Relation::Support,
    ] {
        rules.insert(
            relation.to_string(),
            vec![computed(link_relation, relation)],
        );
    }
}

fn inherit(relation: Relation) -> RelationRule {
    RelationRule::Inherit(relation.to_string())
}

fn computed(tuple_relation: Relation, target_relation: Relation) -> RelationRule {
    RelationRule::Computed {
        tuple_relation: tuple_relation.to_string(),
        target_relation: target_relation.to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GraphNode {
    object: Object,
    relation: Option<String>,
}

#[derive(Debug, Clone)]
struct ExplainIndex {
    tuples_by_node: HashMap<GraphNode, Vec<Tuple>>,
}

impl ExplainIndex {
    fn new(tuples: &[Tuple]) -> Self {
        let mut tuples_by_node = HashMap::new();

        for tuple in tuples {
            let node = GraphNode {
                object: tuple.object.clone(),
                relation: Some(tuple.relation.clone()),
            };
            tuples_by_node
                .entry(node)
                .or_insert_with(Vec::new)
                .push(tuple.clone());
        }

        Self { tuples_by_node }
    }

    fn tuples_for(&self, node: &GraphNode) -> &[Tuple] {
        self.tuples_by_node
            .get(node)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

enum Evaluation {
    Allowed(Vec<ExplainStep>),
    Denied(DeniedExplanation),
}

fn build_capability_explanation<P>(
    package: &P,
    tuples: &[Tuple],
    subject: &DefaultSubject,
    capability: Capability,
    object: &Entity,
    options: ExplainOptions,
) -> Result<CapabilityExplanation, DavendaAuthError>
where
    P: AuthModelPackage,
{
    let binding = package.resolve_binding(capability, object)?.clone();
    let trace = build_relation_trace(
        package.schema(),
        tuples,
        subject,
        binding.relation,
        object,
        options,
    )?;
    let decision = match &trace {
        ExplainTrace::Allowed(_) => ExplainDecision::Allow,
        ExplainTrace::Denied(_) => ExplainDecision::Deny,
    };

    Ok(CapabilityExplanation {
        manifest: package.manifest().clone(),
        subject: subject.clone(),
        capability,
        object: object.clone(),
        binding,
        decision,
        options: options.normalized(),
        trace,
    })
}

fn build_relation_trace(
    schema: &Schema,
    tuples: &[Tuple],
    subject: &DefaultSubject,
    relation: Relation,
    object: &Entity,
    options: ExplainOptions,
) -> Result<ExplainTrace, DavendaAuthError> {
    let options = options.normalized();
    let root = GraphNode {
        object: object.to_object(),
        relation: Some(relation.to_string()),
    };
    let subject = subject.to_subject();
    let index = ExplainIndex::new(tuples);
    let mut visiting = HashSet::new();

    match explain_node(schema, &index, &subject, &root, 1, options, &mut visiting)? {
        Evaluation::Allowed(mut steps) => {
            steps.insert(
                0,
                ExplainStep::Start {
                    node: typed_node(&root)?,
                },
            );
            Ok(ExplainTrace::Allowed(AllowedExplanation { steps }))
        }
        Evaluation::Denied(denied) => Ok(ExplainTrace::Denied(denied)),
    }
}

fn explain_node(
    schema: &Schema,
    index: &ExplainIndex,
    subject: &Subject,
    node: &GraphNode,
    depth: usize,
    options: ExplainOptions,
    visiting: &mut HashSet<GraphNode>,
) -> Result<Evaluation, DavendaAuthError> {
    if options.cycle_protection && !visiting.insert(node.clone()) {
        return Ok(Evaluation::Denied(DeniedExplanation {
            node: typed_node(node)?,
            reason: DeniedReason::CycleDetected,
            attempts: Vec::new(),
        }));
    }

    let result = explain_node_inner(schema, index, subject, node, depth, options, visiting);

    if options.cycle_protection {
        visiting.remove(node);
    }

    result
}

fn explain_node_inner(
    schema: &Schema,
    index: &ExplainIndex,
    subject: &Subject,
    node: &GraphNode,
    depth: usize,
    options: ExplainOptions,
    visiting: &mut HashSet<GraphNode>,
) -> Result<Evaluation, DavendaAuthError> {
    if subject_matches_node(node, subject) {
        return Ok(Evaluation::Allowed(vec![ExplainStep::DirectSubjectMatch {
            node: typed_node(node)?,
        }]));
    }

    for tuple in index.tuples_for(node) {
        if subject_matches_subject(&tuple.subject, subject) {
            return Ok(Evaluation::Allowed(vec![ExplainStep::TupleSubjectMatch {
                from: typed_node(node)?,
                tuple: typed_tuple(tuple)?,
            }]));
        }
    }

    if depth >= options.max_depth {
        return Ok(Evaluation::Denied(DeniedExplanation {
            node: typed_node(node)?,
            reason: DeniedReason::RecursionLimitReached {
                max_depth: options.max_depth,
            },
            attempts: Vec::new(),
        }));
    }

    let mut attempts = Vec::new();

    for rule in inherit_rules_for(schema, node)? {
        let next = GraphNode {
            object: node.object.clone(),
            relation: Some(rule.to_string()),
        };
        let step = ExplainStep::Inherit {
            from: typed_node(node)?,
            to: typed_node(&next)?,
        };

        match explain_node(schema, index, subject, &next, depth + 1, options, visiting)? {
            Evaluation::Allowed(mut steps) => {
                steps.insert(0, step);
                return Ok(Evaluation::Allowed(steps));
            }
            Evaluation::Denied(result) => {
                attempts.push(DeniedAttempt::Inherit {
                    step,
                    result: Box::new(result),
                });
            }
        }
    }

    for tuple in index.tuples_for(node) {
        let next = graph_node_from_subject(&tuple.subject);
        let step = ExplainStep::TupleTraversal {
            from: typed_node(node)?,
            tuple: typed_tuple(tuple)?,
            to: typed_node(&next)?,
        };

        match explain_node(schema, index, subject, &next, depth + 1, options, visiting)? {
            Evaluation::Allowed(mut steps) => {
                steps.insert(0, step);
                return Ok(Evaluation::Allowed(steps));
            }
            Evaluation::Denied(result) => {
                attempts.push(DeniedAttempt::TupleTraversal {
                    step,
                    result: Box::new(result),
                });
            }
        }
    }

    for (tuple_relation, target_relation, is_tuple_to_userset) in
        userset_jump_rules_for(schema, node)?
    {
        let jump_node = GraphNode {
            object: node.object.clone(),
            relation: Some(tuple_relation.to_string()),
        };

        for tuple in index.tuples_for(&jump_node) {
            let next = graph_node_from_subject_with_relation(&tuple.subject, target_relation);
            let step = if is_tuple_to_userset {
                ExplainStep::TupleToUserset {
                    from: typed_node(node)?,
                    via_tuple: typed_tuple(tuple)?,
                    to: typed_node(&next)?,
                }
            } else {
                ExplainStep::Computed {
                    from: typed_node(node)?,
                    via_tuple: typed_tuple(tuple)?,
                    to: typed_node(&next)?,
                }
            };

            match explain_node(schema, index, subject, &next, depth + 1, options, visiting)? {
                Evaluation::Allowed(mut steps) => {
                    steps.insert(0, step);
                    return Ok(Evaluation::Allowed(steps));
                }
                Evaluation::Denied(result) => {
                    attempts.push(if is_tuple_to_userset {
                        DeniedAttempt::TupleToUserset {
                            step,
                            result: Box::new(result),
                        }
                    } else {
                        DeniedAttempt::Computed {
                            step,
                            result: Box::new(result),
                        }
                    });
                }
            }
        }
    }

    Ok(Evaluation::Denied(DeniedExplanation {
        node: typed_node(node)?,
        reason: DeniedReason::NoMatchingPath,
        attempts,
    }))
}

fn inherit_rules_for(schema: &Schema, node: &GraphNode) -> Result<Vec<Relation>, DavendaAuthError> {
    let Some(relation) = node.relation.as_deref() else {
        return Ok(Vec::new());
    };

    let Some(config) = schema.namespaces.get(node.object.namespace.as_str()) else {
        return Ok(Vec::new());
    };

    let Some(rules) = config.rules.get(relation) else {
        return Ok(Vec::new());
    };

    let mut parsed = Vec::new();
    for rule in rules {
        if let RelationRule::Inherit(target_relation) = rule {
            parsed.push(parse_relation(target_relation)?);
        }
    }
    Ok(parsed)
}

fn userset_jump_rules_for(
    schema: &Schema,
    node: &GraphNode,
) -> Result<Vec<(Relation, Relation, bool)>, DavendaAuthError> {
    let Some(relation) = node.relation.as_deref() else {
        return Ok(Vec::new());
    };

    let Some(config) = schema.namespaces.get(node.object.namespace.as_str()) else {
        return Ok(Vec::new());
    };

    let Some(rules) = config.rules.get(relation) else {
        return Ok(Vec::new());
    };

    let mut parsed = Vec::new();
    for rule in rules {
        match rule {
            RelationRule::Computed {
                tuple_relation,
                target_relation,
            } => parsed.push((
                parse_relation(tuple_relation)?,
                parse_relation(target_relation)?,
                false,
            )),
            RelationRule::TupleToUserset {
                tuple_relation,
                target_relation,
            } => parsed.push((
                parse_relation(tuple_relation)?,
                parse_relation(target_relation)?,
                true,
            )),
            RelationRule::Inherit(_) => {}
        }
    }
    Ok(parsed)
}

fn graph_node_from_subject(subject: &Subject) -> GraphNode {
    match subject {
        Subject::Entity(object) => GraphNode {
            object: object.clone(),
            relation: None,
        },
        Subject::Userset { object, relation } => GraphNode {
            object: object.clone(),
            relation: Some(relation.clone()),
        },
    }
}

fn graph_node_from_subject_with_relation(subject: &Subject, relation: Relation) -> GraphNode {
    let object = match subject {
        Subject::Entity(object) | Subject::Userset { object, .. } => object.clone(),
    };

    GraphNode {
        object,
        relation: Some(relation.to_string()),
    }
}

fn typed_node(node: &GraphNode) -> Result<ExplainedNode, DavendaAuthError> {
    let object = Entity::from_object(&node.object).ok_or_else(|| {
        DavendaAuthError::UnsupportedExplainNamespace {
            namespace: node.object.namespace.clone(),
        }
    })?;
    let relation = match &node.relation {
        Some(relation) => Some(parse_relation(relation)?),
        None => None,
    };

    Ok(ExplainedNode { object, relation })
}

fn typed_tuple(tuple: &Tuple) -> Result<DefaultTuple, DavendaAuthError> {
    DefaultTuple::from_tuple(tuple).ok_or_else(|| {
        if Entity::from_object(&tuple.object).is_none() {
            DavendaAuthError::UnsupportedExplainNamespace {
                namespace: tuple.object.namespace.clone(),
            }
        } else {
            DavendaAuthError::UnsupportedExplainRelation {
                relation: tuple.relation.clone(),
            }
        }
    })
}

fn parse_relation(relation: &str) -> Result<Relation, DavendaAuthError> {
    Relation::from_str(relation).ok_or_else(|| DavendaAuthError::UnsupportedExplainRelation {
        relation: relation.to_string(),
    })
}

fn subject_matches_node(node: &GraphNode, subject: &Subject) -> bool {
    match subject {
        Subject::Entity(object) => {
            relation_matches(node.relation.as_deref(), None) && object_matches(&node.object, object)
        }
        Subject::Userset { object, relation } => {
            relation_matches(node.relation.as_deref(), Some(relation.as_str()))
                && object_matches(&node.object, object)
        }
    }
}

fn subject_matches_subject(candidate: &Subject, target: &Subject) -> bool {
    match (candidate, target) {
        (Subject::Entity(left), Subject::Entity(right)) => object_matches(left, right),
        (
            Subject::Userset {
                object: left_object,
                relation: left_relation,
            },
            Subject::Userset {
                object: right_object,
                relation: right_relation,
            },
        ) => object_matches(left_object, right_object) && left_relation == right_relation,
        _ => false,
    }
}

fn object_matches(candidate: &Object, target: &Object) -> bool {
    (candidate.namespace == target.namespace || candidate.namespace == "*")
        && (candidate.id == target.id || candidate.id == "*")
}

fn relation_matches(candidate: Option<&str>, target: Option<&str>) -> bool {
    candidate == target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_converts_to_object() {
        let site = Entity::site("london");
        let object = site.to_object();

        assert_eq!(object.namespace, "site");
        assert_eq!(object.id, "london");
        assert_eq!(site.to_string(), "site:london");
    }

    #[test]
    fn typed_subject_and_tuple_convert_to_zanzibar_types() {
        let team = Entity::team("ops");
        let asset = Entity::asset("hero-image");
        let subject = DefaultSubject::userset(team.clone(), Relation::Member);
        let tuple = DefaultTuple::new(asset.clone(), Relation::Viewer, subject.clone());

        assert_eq!(
            subject.to_subject(),
            Subject::Userset {
                object: team.to_object(),
                relation: "member".into(),
            }
        );
        assert_eq!(
            tuple.to_tuple(),
            Tuple {
                object: asset.to_object(),
                relation: "viewer".into(),
                subject: Subject::Userset {
                    object: Entity::team("ops").to_object(),
                    relation: "member".into(),
                },
            }
        );
    }

    #[test]
    fn default_manifest_tracks_independent_versions() {
        let manifest = default_manifest();

        assert_eq!(manifest.name, "platform-default-auth");
        assert_eq!(manifest.version.to_string(), "1.0.0");
        assert_eq!(manifest.mode, PackageMode::Replace);
        assert_eq!(manifest.storage_schema_version, 1);
        assert_eq!(manifest.model_version, 1);
        assert_eq!(manifest.capability_binding_version, 1);
    }

    #[test]
    fn default_schema_contains_expected_default_namespaces() {
        let schema = default_schema();

        for namespace in [
            Namespace::Tenant,
            Namespace::Site,
            Namespace::Brand,
            Namespace::Storefront,
            Namespace::Page,
            Namespace::Navigation,
            Namespace::Product,
            Namespace::Collection,
            Namespace::Order,
            Namespace::Subscription,
            Namespace::MembershipTier,
            Namespace::Event,
            Namespace::EventSlot,
            Namespace::Booking,
            Namespace::Media,
            Namespace::MediaLibrary,
            Namespace::Asset,
            Namespace::AssetFolder,
            Namespace::ThemeAssetBundle,
            Namespace::AdminModule,
        ] {
            assert!(schema.namespaces.contains_key(namespace.as_str()));
        }
    }

    #[test]
    fn page_namespace_inherits_roles_from_site() {
        let schema = default_schema();
        let page_namespace = schema.namespaces.get("page").unwrap();

        assert_eq!(
            page_namespace.rules.get("viewer"),
            Some(&vec![RelationRule::Computed {
                tuple_relation: "site".into(),
                target_relation: "viewer".into(),
            }])
        );
        assert_eq!(
            page_namespace.rules.get("publish"),
            Some(&vec![
                RelationRule::Inherit("manage".into()),
                RelationRule::Inherit("editor".into()),
            ])
        );
    }

    #[test]
    fn asset_namespace_contains_storage_and_publication_rules() {
        let schema = default_schema();
        let asset_namespace = schema.namespaces.get("asset").unwrap();

        assert_eq!(
            asset_namespace.rules.get("replace"),
            Some(&vec![RelationRule::Inherit("edit".into())])
        );
        assert_eq!(
            asset_namespace.rules.get("manage_storage"),
            Some(&vec![RelationRule::Inherit("manage".into())])
        );
        assert!(
            !asset_namespace.rules.contains_key("read_public"),
            "read_public is intentionally left as a direct relation so publication state can be written explicitly"
        );
    }

    #[test]
    fn storefront_and_order_namespaces_include_commerce_permissions() {
        let schema = default_schema();
        let storefront = schema.namespaces.get("storefront").unwrap();
        let order = schema.namespaces.get("order").unwrap();

        assert_eq!(
            storefront.rules.get("checkout"),
            Some(&vec![
                RelationRule::Inherit("view".into()),
                RelationRule::Inherit("member".into()),
            ])
        );
        assert_eq!(
            order.rules.get("refund"),
            Some(&vec![
                RelationRule::Inherit("manage".into()),
                RelationRule::Inherit("support".into()),
            ])
        );
    }

    #[test]
    fn capability_registry_contains_expected_bindings() {
        let package = DefaultAuthModelPackage::default();

        let page_publish = package.binding_for(Capability::CmsPagePublish).unwrap();
        assert_eq!(page_publish.resource_namespaces, vec![Namespace::Page]);
        assert_eq!(page_publish.relation, Relation::Publish);

        let booking_create = package
            .binding_for(Capability::EventsBookingCreate)
            .unwrap();
        assert_eq!(
            booking_create.resource_namespaces,
            vec![Namespace::EventSlot]
        );
        assert_eq!(booking_create.relation, Relation::Book);
    }

    #[test]
    fn capability_resolution_rejects_wrong_namespace() {
        let package = DefaultAuthModelPackage::default();
        let result = package.resolve_binding(Capability::CmsPagePublish, &Entity::product("sku-1"));

        match result {
            Err(DavendaAuthError::ResourceNamespaceMismatch {
                capability,
                actual,
                expected,
            }) => {
                assert_eq!(capability, Capability::CmsPagePublish);
                assert_eq!(actual, Namespace::Product);
                assert_eq!(expected, vec![Namespace::Page]);
            }
            other => panic!("expected namespace mismatch, got {other:?}"),
        }
    }

    #[test]
    fn explain_capability_returns_allow_path_for_inherited_access() {
        let package = DefaultAuthModelPackage::default();
        let page = Entity::page("homepage");
        let site = Entity::site("main");
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let tuples = vec![
            DefaultTuple::new(page.clone(), Relation::Site, site.clone().as_subject()).to_tuple(),
            DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone()).to_tuple(),
        ];

        let explanation = build_capability_explanation(
            &package,
            &tuples,
            &subject,
            Capability::CmsPageRead,
            &page,
            ExplainOptions::default(),
        )
        .unwrap();

        assert_eq!(explanation.decision, ExplainDecision::Allow);
        assert_eq!(explanation.binding.relation, Relation::View);

        let ExplainTrace::Allowed(trace) = explanation.trace else {
            panic!("expected allow trace");
        };
        assert_eq!(
            trace.steps.first(),
            Some(&ExplainStep::Start {
                node: ExplainedNode {
                    object: page.clone(),
                    relation: Some(Relation::View),
                },
            })
        );
        assert!(trace.steps.iter().any(|step| matches!(
            step,
            ExplainStep::Computed {
                from,
                via_tuple,
                to,
            } if *from == ExplainedNode {
                object: page.clone(),
                relation: Some(Relation::Viewer),
            } && *via_tuple == DefaultTuple::new(
                page.clone(),
                Relation::Site,
                site.clone().as_subject(),
            ) && *to == ExplainedNode {
                object: site.clone(),
                relation: Some(Relation::Viewer),
            }
        )));
        assert!(trace.steps.iter().any(|step| matches!(
            step,
            ExplainStep::TupleSubjectMatch { from, tuple }
                if *from == ExplainedNode {
                    object: site.clone(),
                    relation: Some(Relation::Viewer),
                } && *tuple == DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone())
        )));
    }

    #[test]
    fn explain_capability_returns_deny_tree_when_required_publish_path_is_missing() {
        let package = DefaultAuthModelPackage::default();
        let page = Entity::page("homepage");
        let site = Entity::site("main");
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let tuples = vec![
            DefaultTuple::new(page.clone(), Relation::Site, site.clone().as_subject()).to_tuple(),
            DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone()).to_tuple(),
        ];

        let explanation = build_capability_explanation(
            &package,
            &tuples,
            &subject,
            Capability::CmsPagePublish,
            &page,
            ExplainOptions::default(),
        )
        .unwrap();

        assert_eq!(explanation.decision, ExplainDecision::Deny);

        let ExplainTrace::Denied(trace) = explanation.trace else {
            panic!("expected deny trace");
        };
        assert_eq!(
            trace.node,
            ExplainedNode {
                object: page.clone(),
                relation: Some(Relation::Publish),
            }
        );
        assert_eq!(trace.reason, DeniedReason::NoMatchingPath);
        assert_eq!(trace.attempts.len(), 2);
        assert!(
            trace
                .attempts
                .iter()
                .all(|attempt| matches!(attempt, DeniedAttempt::Inherit { .. }))
        );
    }

    #[test]
    fn explain_capability_honors_public_wildcard_subjects() {
        let package = DefaultAuthModelPackage::default();
        let asset = Entity::asset("hero");
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let tuples = vec![
            DefaultTuple::new(
                asset.clone(),
                Relation::ReadPublic,
                DefaultSubject::entity(Entity::any_user()),
            )
            .to_tuple(),
        ];

        let explanation = build_capability_explanation(
            &package,
            &tuples,
            &subject,
            Capability::AssetReadPublic,
            &asset,
            ExplainOptions::default(),
        )
        .unwrap();

        assert_eq!(explanation.decision, ExplainDecision::Allow);

        let ExplainTrace::Allowed(trace) = explanation.trace else {
            panic!("expected allow trace");
        };
        assert!(trace.steps.iter().any(|step| matches!(
            step,
            ExplainStep::TupleSubjectMatch { tuple, .. }
                if *tuple == DefaultTuple::new(
                    asset.clone(),
                    Relation::ReadPublic,
                    DefaultSubject::entity(Entity::any_user()),
                )
        )));
    }

    #[test]
    fn explain_capability_reports_recursion_limit_when_depth_budget_is_exhausted() {
        let package = DefaultAuthModelPackage::default();
        let page = Entity::page("homepage");
        let site = Entity::site("main");
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let tuples = vec![
            DefaultTuple::new(page.clone(), Relation::Site, site.clone().as_subject()).to_tuple(),
            DefaultTuple::new(site.clone(), Relation::Viewer, subject.clone()).to_tuple(),
        ];

        let explanation = build_capability_explanation(
            &package,
            &tuples,
            &subject,
            Capability::CmsPageRead,
            &page,
            ExplainOptions::new(1),
        )
        .unwrap();

        assert_eq!(explanation.decision, ExplainDecision::Deny);

        let ExplainTrace::Denied(trace) = explanation.trace else {
            panic!("expected deny trace");
        };
        assert_eq!(
            trace.reason,
            DeniedReason::RecursionLimitReached { max_depth: 1 }
        );
    }
}
