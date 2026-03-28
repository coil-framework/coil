use crate::error::MediaModelError;
use crate::validation::validate_token;
use std::fmt;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, MediaModelError> {
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

token_type!(MediaLibraryId, "media_library_id");
token_type!(MediaFolderId, "media_folder_id");
token_type!(MediaAssetId, "media_asset_id");
token_type!(MediaRevisionId, "media_revision_id");
token_type!(MediaDerivativeId, "media_derivative_id");
token_type!(MediaSlug, "media_slug");
token_type!(MediaTag, "media_tag");
token_type!(MediaFieldId, "media_field_id");
