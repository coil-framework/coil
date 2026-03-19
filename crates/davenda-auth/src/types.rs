use super::*;

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
