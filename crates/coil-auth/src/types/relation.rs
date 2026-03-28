use super::*;

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
    Merchandiser,
    View,
    Edit,
    Publish,
    Manage,
    FeaturedEdit,
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
            Self::Merchandiser => "merchandiser",
            Self::View => "view",
            Self::Edit => "edit",
            Self::Publish => "publish",
            Self::Manage => "manage",
            Self::FeaturedEdit => "featured_edit",
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
            "merchandiser" => Some(Self::Merchandiser),
            "view" => Some(Self::View),
            "edit" => Some(Self::Edit),
            "publish" => Some(Self::Publish),
            "manage" => Some(Self::Manage),
            "featured_edit" => Some(Self::FeaturedEdit),
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
