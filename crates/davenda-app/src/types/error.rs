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
    #[error("site `{site}` is declared more than once")]
    DuplicateSite { site: String },
    #[error("site domain `{domain}` is declared more than once across sites")]
    DuplicateSiteDomain { domain: String },
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
    #[error("`{field}` is not a valid relative path: `{value}`")]
    InvalidRelativePath { field: &'static str, value: String },
    #[error("customer app `{app_id}` installs extensions but no extension packages were supplied")]
    ExtensionPackagesRequired { app_id: String },
    #[error("default locale `{default_locale}` is not in the supported locale set")]
    DefaultLocaleNotSupported { default_locale: String },
    #[error(
        "site `{site}` default locale `{default_locale}` is not in the site's supported locale set"
    )]
    SiteDefaultLocaleNotSupported {
        site: String,
        default_locale: String,
    },
    #[error(
        "site `{site}` locale `{locale}` is not declared by the customer app supported locale set"
    )]
    SiteLocaleOutsideAppSupport { site: String, locale: String },
    #[error("customer app `{app_id}` must declare at least one canonical domain")]
    MissingCanonicalDomain { app_id: String },
    #[error("customer app `{app_id}` site `{site}` must declare at least one canonical domain")]
    MissingCanonicalSiteDomain { app_id: String, site: String },
    #[error("customer app `{app_id}` must declare a primary site when sites are configured")]
    MissingPrimarySite { app_id: String },
    #[error("customer app `{app_id}` primary site `{site}` is not declared")]
    UnknownPrimarySite { app_id: String, site: String },
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
        "customer app primary site `{manifest}` does not match runtime config primary site `{configured}`"
    )]
    ConfigPrimarySiteMismatch {
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
    #[error(
        "customer app manifest sites differ from runtime config sites; manifest-only={manifest_only:?}, config-only={configured_only:?}"
    )]
    ConfigSitesMismatch {
        manifest_only: Vec<String>,
        configured_only: Vec<String>,
    },
    #[error(
        "customer app site `{site}` field `{field}` differs from runtime config; manifest=`{manifest}`, config=`{configured}`"
    )]
    ConfigSiteFieldMismatch {
        site: String,
        field: &'static str,
        manifest: String,
        configured: String,
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
    #[error("failed to read customer app manifest `{path}`: {message}")]
    ManifestRead { path: String, message: String },
    #[error("failed to parse customer app manifest: {message}")]
    ManifestParse { message: String },
    #[error("{0}")]
    ModuleCapabilityValidation(#[from] CapabilityValidationError),
    #[error("{0}")]
    Report(#[from] ReportModelError),
    #[error("{0}")]
    Assets(#[from] davenda_assets::AssetModelError),
    #[error("{0}")]
    Wasm(#[from] WasmModelError),
    #[error("{message}")]
    RuntimeBuild { message: String },
}
