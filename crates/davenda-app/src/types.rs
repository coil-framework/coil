use super::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("`{field}` has invalid hostname `{value}`")]
    InvalidHostname { field: &'static str, value: String },
    #[error("module `{module}` is installed more than once")]
    DuplicateInstalledModule { module: String },
    #[error("domain `{domain}` is declared more than once")]
    DuplicateDomain { domain: String },
    #[error("theme namespace `{namespace}` is declared more than once")]
    DuplicateThemeNamespace { namespace: String },
    #[error("content model `{model_id}` is declared more than once")]
    DuplicateContentModel { model_id: String },
    #[error("content model `{model_id}` declares duplicate field `{field_id}`")]
    DuplicateContentField { model_id: String, field_id: String },
    #[error("extension `{extension_id}` is declared more than once")]
    DuplicateExtension { extension_id: String },
    #[error("customer app `{app_id}` installs extensions but no extension packages were supplied")]
    ExtensionPackagesRequired { app_id: String },
    #[error("default locale `{default_locale}` is not in the supported locale set")]
    DefaultLocaleNotSupported { default_locale: String },
    #[error("customer app `{app_id}` must declare at least one canonical domain")]
    MissingCanonicalDomain { app_id: String },
    #[error("customer app `{app_id}` does not install module `{module}`")]
    UnknownInstalledModule { app_id: String, module: String },
    #[error("module `{module}` requires installed dependency `{dependency}`")]
    MissingModuleDependency { module: String, dependency: String },
    #[error(
        "customer app `{app_id}` configures auth package `{configured}` but runtime package `{actual}` was supplied"
    )]
    AuthPackageMismatch {
        app_id: String,
        configured: String,
        actual: String,
    },
    #[error("customer app manifest `{manifest}` does not match runtime config app `{configured}`")]
    ConfigAppMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest auth package `{manifest}` does not match runtime config auth package `{configured}`"
    )]
    ConfigAuthPackageMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest default locale `{manifest}` does not match runtime config default locale `{configured}`"
    )]
    ConfigDefaultLocaleMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest supported locales `{manifest:?}` do not match runtime config supported locales `{configured:?}`"
    )]
    ConfigSupportedLocalesMismatch {
        manifest: Vec<String>,
        configured: Vec<String>,
    },
    #[error(
        "customer app canonical host `{manifest}` does not match runtime config canonical host `{configured}`"
    )]
    ConfigCanonicalHostMismatch {
        manifest: String,
        configured: String,
    },
    #[error(
        "customer app manifest modules differ from runtime config modules; manifest-only={manifest_only:?}, config-only={configured_only:?}"
    )]
    ConfigModulesMismatch {
        manifest_only: Vec<String>,
        configured_only: Vec<String>,
    },
    #[error("runtime provided modules not installed by customer app `{app_id}`: {modules:?}")]
    UnexpectedRuntimeModules {
        app_id: String,
        modules: Vec<String>,
    },
    #[error(
        "extension `{extension_id}` is installed for customer app `{extension_customer_app}` but manifest is `{app_id}`"
    )]
    ExtensionCustomerAppMismatch {
        extension_id: String,
        extension_customer_app: String,
        app_id: String,
    },
    #[error("customer app `{app_id}` does not have a package for extension `{extension_id}`")]
    UnknownExtensionPackage {
        app_id: String,
        extension_id: String,
    },
    #[error(
        "customer extension `{extension_id}` pins version `{configured}` but package provides `{actual}`"
    )]
    ExtensionVersionMismatch {
        extension_id: String,
        configured: ContractVersion,
        actual: ContractVersion,
    },
    #[error(
        "customer extension `{extension_id}` pins artifact digest `{configured}` but package provides `{actual}`"
    )]
    ExtensionArtifactChecksumMismatch {
        extension_id: String,
        configured: String,
        actual: String,
    },
    #[error("{0}")]
    ModuleCapabilityValidation(#[from] CapabilityValidationError),
    #[error("{0}")]
    Cli(#[from] CliModelError),
    #[error("{0}")]
    Wasm(#[from] WasmModelError),
    #[error("{message}")]
    RuntimeBuild { message: String },
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeProfile {
    pub active: ThemeId,
    pub template_namespaces: Vec<TemplateNamespace>,
    pub asset_roots: Vec<String>,
}

impl ThemeProfile {
    pub fn new(
        active: ThemeId,
        template_namespaces: Vec<TemplateNamespace>,
    ) -> Result<Self, AppModelError> {
        if template_namespaces.is_empty() {
            return Err(AppModelError::EmptyField {
                field: "template_namespaces",
            });
        }

        let mut seen = BTreeSet::new();
        for namespace in &template_namespaces {
            if !seen.insert(namespace.to_string()) {
                return Err(AppModelError::DuplicateThemeNamespace {
                    namespace: namespace.to_string(),
                });
            }
        }

        Ok(Self {
            active,
            template_namespaces,
            asset_roots: Vec::new(),
        })
    }

    pub fn with_asset_root(mut self, asset_root: impl Into<String>) -> Result<Self, AppModelError> {
        self.asset_roots
            .push(require_non_empty("theme_asset_root", asset_root.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentFieldType {
    Text,
    RichText,
    Slug,
    Boolean,
    Integer,
    DateTime,
    Asset,
    Reference,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentField {
    pub id: ContentFieldId,
    pub field_type: ContentFieldType,
    pub localized: bool,
    pub required: bool,
}

impl ContentField {
    pub fn new(id: impl Into<String>, field_type: ContentFieldType) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ContentFieldId::new(id.into())?,
            field_type,
            localized: false,
            required: false,
        })
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentModel {
    pub id: ContentModelId,
    pub resource_kind: String,
    pub fields: Vec<ContentField>,
}

impl ContentModel {
    pub fn new(
        id: impl Into<String>,
        resource_kind: impl Into<String>,
        fields: Vec<ContentField>,
    ) -> Result<Self, AppModelError> {
        if fields.is_empty() {
            return Err(AppModelError::EmptyField {
                field: "content_model_fields",
            });
        }

        let id = ContentModelId::new(id.into())?;
        let resource_kind = require_non_empty("content_model_resource_kind", resource_kind.into())?;
        let mut seen = BTreeSet::new();
        for field in &fields {
            if !seen.insert(field.id.to_string()) {
                return Err(AppModelError::DuplicateContentField {
                    model_id: id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            resource_kind,
            fields,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Extend,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthStrategy {
    pub mode: AuthMode,
    pub package_name: String,
}

impl AuthStrategy {
    pub fn new(mode: AuthMode, package_name: impl Into<String>) -> Result<Self, AppModelError> {
        Ok(Self {
            mode,
            package_name: require_non_empty("auth_package_name", package_name.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerExtension {
    pub id: ExtensionId,
    pub package_version: ContractVersion,
    pub artifact_sha256: String,
    pub config: BTreeMap<String, ExtensionConfigValue>,
    pub installation: ExtensionInstallation,
}

impl CustomerExtension {
    pub fn new(
        id: impl Into<String>,
        package_version: ContractVersion,
        artifact_sha256: impl Into<String>,
        installation: ExtensionInstallation,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ExtensionId::new(id.into())?,
            package_version,
            artifact_sha256: validate_sha256("extension_artifact_sha256", artifact_sha256.into())?,
            config: BTreeMap::new(),
            installation,
        })
    }

    pub fn with_config_value(
        mut self,
        key: impl Into<String>,
        value: ExtensionConfigValue,
    ) -> Result<Self, AppModelError> {
        let key = validate_token("extension_config_key", key.into())?;
        self.config.insert(key, value);
        Ok(self)
    }
}
