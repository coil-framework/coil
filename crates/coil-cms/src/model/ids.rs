use super::*;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(String);

impl PageId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("page_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("revision_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

token_type!(NavigationId, "navigation_id");
token_type!(BlockTypeId, "block_type_id");
token_type!(BlockFieldId, "block_field_id");
token_type!(BlockInstanceId, "block_instance_id");
token_type!(SharedBlockId, "shared_block_id");

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationItemId(String);

impl NavigationItemId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("navigation_item_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NavigationItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocaleCode(String);

impl LocaleCode {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("locale", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LocaleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("slug", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateHandle(String);

impl TemplateHandle {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("template_handle", value.into())?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetReference(String);

impl AssetReference {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("asset_reference", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
