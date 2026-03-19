use super::*;

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
