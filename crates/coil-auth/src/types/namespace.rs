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
