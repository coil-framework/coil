use super::*;

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AppModelError> {
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

token_type!(CustomerAppId, "customer_app_id");
token_type!(SiteId, "site_id");
token_type!(ThemeId, "theme_id");
token_type!(ContentModelId, "content_model_id");
token_type!(ContentFieldId, "content_field_id");
token_type!(ExtensionId, "extension_id");
token_type!(ModuleId, "module_id");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDomain {
    pub hostname: String,
    pub canonical: bool,
}

impl AppDomain {
    pub fn new(hostname: impl Into<String>, canonical: bool) -> Result<Self, AppModelError> {
        Ok(Self {
            hostname: validate_hostname("domain_hostname", hostname.into())?,
            canonical,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModuleSpec {
    pub id: ModuleId,
    pub version_req: Option<String>,
}

impl InstalledModuleSpec {
    pub fn new(id: impl Into<String>) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ModuleId::new(id.into())?,
            version_req: None,
        })
    }

    pub fn pinned(mut self, version_req: impl Into<String>) -> Result<Self, AppModelError> {
        self.version_req = Some(require_non_empty("module_version_req", version_req.into())?);
        Ok(self)
    }
}
