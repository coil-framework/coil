use std::collections::BTreeSet;
use std::fmt;

use davenda_auth::AuthModelPackage;
use davenda_core::{
    AdminResourceContribution, BulkOperationDefinition, CapabilityValidationError,
    CoreServiceDependency, EventSubscription, JobContract, MigrationContract, ModuleDependencyKind,
    ModuleManifest, ReportDefinition, RouteSurface, SearchIndexContribution,
    validate_module_capabilities,
};
use davenda_i18n::LocaleTag;
use davenda_template::TemplateNamespace;
use davenda_wasm::ExtensionInstallation;
use thiserror::Error;

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
    #[error(
        "extension `{extension_id}` is installed for customer app `{extension_customer_app}` but manifest is `{app_id}`"
    )]
    ExtensionCustomerAppMismatch {
        extension_id: String,
        extension_customer_app: String,
        app_id: String,
    },
    #[error("{0}")]
    ModuleCapabilityValidation(#[from] CapabilityValidationError),
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
    pub installation: ExtensionInstallation,
}

impl CustomerExtension {
    pub fn new(
        id: impl Into<String>,
        installation: ExtensionInstallation,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ExtensionId::new(id.into())?,
            installation,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppManifest {
    pub id: CustomerAppId,
    pub display_name: String,
    pub domains: Vec<AppDomain>,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub modules: Vec<InstalledModuleSpec>,
    pub theme: ThemeProfile,
    pub auth: AuthStrategy,
    pub content_models: Vec<ContentModel>,
    pub customer_migrations: Vec<MigrationContract>,
    pub extensions: Vec<CustomerExtension>,
}

impl CustomerAppManifest {
    pub fn new(
        id: CustomerAppId,
        display_name: impl Into<String>,
        default_locale: LocaleTag,
        supported_locales: Vec<LocaleTag>,
        theme: ThemeProfile,
        auth: AuthStrategy,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id,
            display_name: require_non_empty("display_name", display_name.into())?,
            domains: Vec::new(),
            default_locale,
            supported_locales,
            modules: Vec::new(),
            theme,
            auth,
            content_models: Vec::new(),
            customer_migrations: Vec::new(),
            extensions: Vec::new(),
        })
    }

    pub fn with_domain(mut self, domain: AppDomain) -> Self {
        self.domains.push(domain);
        self
    }

    pub fn with_module(mut self, module: InstalledModuleSpec) -> Self {
        self.modules.push(module);
        self
    }

    pub fn with_content_model(mut self, model: ContentModel) -> Self {
        self.content_models.push(model);
        self
    }

    pub fn with_customer_migration(mut self, migration: MigrationContract) -> Self {
        self.customer_migrations.push(migration);
        self
    }

    pub fn with_extension(mut self, extension: CustomerExtension) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn validate(&self) -> Result<(), AppModelError> {
        if !self
            .supported_locales
            .iter()
            .any(|locale| locale == &self.default_locale)
        {
            return Err(AppModelError::DefaultLocaleNotSupported {
                default_locale: self.default_locale.to_string(),
            });
        }

        let mut domains = BTreeSet::new();
        let mut canonical_domains = 0usize;
        for domain in &self.domains {
            if !domains.insert(domain.hostname.clone()) {
                return Err(AppModelError::DuplicateDomain {
                    domain: domain.hostname.clone(),
                });
            }
            if domain.canonical {
                canonical_domains += 1;
            }
        }
        if canonical_domains == 0 {
            return Err(AppModelError::MissingCanonicalDomain {
                app_id: self.id.to_string(),
            });
        }

        let mut modules = BTreeSet::new();
        for module in &self.modules {
            if !modules.insert(module.id.to_string()) {
                return Err(AppModelError::DuplicateInstalledModule {
                    module: module.id.to_string(),
                });
            }
        }

        let mut content_models = BTreeSet::new();
        for model in &self.content_models {
            if !content_models.insert(model.id.to_string()) {
                return Err(AppModelError::DuplicateContentModel {
                    model_id: model.id.to_string(),
                });
            }
        }

        let mut extensions = BTreeSet::new();
        for extension in &self.extensions {
            if !extensions.insert(extension.id.to_string()) {
                return Err(AppModelError::DuplicateExtension {
                    extension_id: extension.id.to_string(),
                });
            }

            if extension.installation.customer_app_id != self.id.as_str() {
                return Err(AppModelError::ExtensionCustomerAppMismatch {
                    extension_id: extension.id.to_string(),
                    extension_customer_app: extension.installation.customer_app_id.clone(),
                    app_id: self.id.to_string(),
                });
            }
        }

        Ok(())
    }

    pub fn compose<P>(
        &self,
        auth_package: &P,
        manifests: &[ModuleManifest],
    ) -> Result<CustomerAppComposition, AppModelError>
    where
        P: AuthModelPackage,
    {
        self.validate()?;

        if auth_package.manifest().name != self.auth.package_name {
            return Err(AppModelError::AuthPackageMismatch {
                app_id: self.id.to_string(),
                configured: self.auth.package_name.clone(),
                actual: auth_package.manifest().name.clone(),
            });
        }

        let installed_ids = self
            .modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>();

        let mut selected = Vec::new();
        for module in &self.modules {
            let manifest = manifests
                .iter()
                .find(|candidate| candidate.name == module.id.as_str())
                .ok_or_else(|| AppModelError::UnknownInstalledModule {
                    app_id: self.id.to_string(),
                    module: module.id.to_string(),
                })?;

            validate_module_capabilities(auth_package, manifest)?;
            selected.push(manifest.clone());
        }

        for manifest in &selected {
            for dependency in &manifest.module_dependencies {
                if dependency.kind == ModuleDependencyKind::Required
                    && !installed_ids
                        .iter()
                        .any(|module| module == &dependency.module)
                {
                    return Err(AppModelError::MissingModuleDependency {
                        module: manifest.name.clone(),
                        dependency: dependency.module.clone(),
                    });
                }
            }
        }

        let mut required_core_services = Vec::new();
        let mut migrations = self.customer_migrations.clone();
        let mut route_surfaces = Vec::new();
        let mut jobs = Vec::new();
        let mut event_subscriptions = Vec::new();
        let mut admin_resources = Vec::new();
        let mut search_contributions = Vec::new();
        let mut report_definitions = Vec::new();
        let mut bulk_operations = Vec::new();

        for manifest in &selected {
            for dependency in &manifest.core_service_dependencies {
                if !required_core_services.contains(dependency) {
                    required_core_services.push(*dependency);
                }
            }

            migrations.extend(manifest.migrations.clone());
            route_surfaces.extend(manifest.route_surfaces.clone());
            jobs.extend(manifest.jobs.clone());
            event_subscriptions.extend(manifest.event_subscriptions.clone());
            admin_resources.extend(manifest.admin_resources.clone());
            search_contributions.extend(manifest.search_contributions.clone());
            report_definitions.extend(manifest.report_definitions.clone());
            bulk_operations.extend(manifest.bulk_operations.clone());
        }

        Ok(CustomerAppComposition {
            app_id: self.id.clone(),
            display_name: self.display_name.clone(),
            default_locale: self.default_locale.clone(),
            supported_locales: self.supported_locales.clone(),
            installed_modules: self.modules.clone(),
            required_core_services,
            migrations,
            route_surfaces,
            jobs,
            event_subscriptions,
            admin_resources,
            search_contributions,
            report_definitions,
            bulk_operations,
            theme: self.theme.clone(),
            content_models: self.content_models.clone(),
            extensions: self.extensions.clone(),
            auth: self.auth.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppComposition {
    pub app_id: CustomerAppId,
    pub display_name: String,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub installed_modules: Vec<InstalledModuleSpec>,
    pub required_core_services: Vec<CoreServiceDependency>,
    pub migrations: Vec<MigrationContract>,
    pub route_surfaces: Vec<RouteSurface>,
    pub jobs: Vec<JobContract>,
    pub event_subscriptions: Vec<EventSubscription>,
    pub admin_resources: Vec<AdminResourceContribution>,
    pub search_contributions: Vec<SearchIndexContribution>,
    pub report_definitions: Vec<ReportDefinition>,
    pub bulk_operations: Vec<BulkOperationDefinition>,
    pub theme: ThemeProfile,
    pub content_models: Vec<ContentModel>,
    pub extensions: Vec<CustomerExtension>,
    pub auth: AuthStrategy,
}

fn validate_token(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(AppModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn validate_hostname(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        && trimmed.contains('.')
    {
        Ok(trimmed)
    } else {
        Err(AppModelError::InvalidHostname {
            field,
            value: trimmed,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use davenda_auth::{Capability, DefaultAuthModelPackage};
    use davenda_core::{
        AdminContributionKind, AdminNavigationSection, BulkOperationKind, BulkOperationScope,
        CapabilityContract, ReportDeliveryMode, ReportFormat, ReportSensitivity,
        SearchDocumentKind, SearchFieldContribution, SearchFieldRole, SearchIndexContribution,
        SearchInvalidationRule, SearchInvalidationTrigger, SearchRebuildStrategy, SearchVisibility,
    };
    use davenda_wasm::{HandlerId, HandlerInstallation, HostCapabilityGrant, HostGrantSet};

    fn locale(value: &str) -> LocaleTag {
        LocaleTag::new(value).expect("locale is valid")
    }

    fn theme() -> ThemeProfile {
        ThemeProfile::new(
            ThemeId::new("harbor").unwrap(),
            vec![
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateNamespace::new("harbor").unwrap(),
            ],
        )
        .unwrap()
        .with_asset_root("theme/assets")
        .unwrap()
    }

    fn auth() -> AuthStrategy {
        AuthStrategy::new(
            AuthMode::Extend,
            DefaultAuthModelPackage::default().manifest().name.clone(),
        )
        .unwrap()
    }

    fn app() -> CustomerAppManifest {
        CustomerAppManifest::new(
            CustomerAppId::new("harbor-shop").unwrap(),
            "Harbor Shop",
            locale("en-GB"),
            vec![locale("en-GB"), locale("fr-FR")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("shop.example.com", true).unwrap())
        .with_module(InstalledModuleSpec::new("cms").unwrap())
        .with_module(InstalledModuleSpec::new("commerce").unwrap())
        .with_content_model(
            ContentModel::new(
                "landing_page",
                "page",
                vec![
                    ContentField::new("title", ContentFieldType::Text)
                        .unwrap()
                        .localized()
                        .required(),
                    ContentField::new("hero_image", ContentFieldType::Asset).unwrap(),
                ],
            )
            .unwrap(),
        )
        .with_customer_migration(MigrationContract::new(
            "customer.content",
            90,
            "Creates customer app landing-page projections",
        ))
        .with_extension(
            CustomerExtension::new(
                "loyalty-widget",
                ExtensionInstallation::new(
                    "harbor-shop",
                    vec![HandlerInstallation::new(
                        HandlerId::new("account.loyalty.widget").unwrap(),
                        HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                            slot: "account.summary".to_string(),
                        }]),
                    )],
                )
                .unwrap(),
            )
            .unwrap(),
        )
    }

    fn module_manifests() -> Vec<ModuleManifest> {
        vec![
            ModuleManifest::new("cms")
                .with_optional_capabilities(vec![Capability::CmsPagePublish])
                .with_capability_contracts(vec![CapabilityContract::optional(
                    Capability::CmsPagePublish,
                    ["page"],
                )])
                .with_core_service_dependencies(vec![CoreServiceDependency::Seo])
                .with_migrations(vec![MigrationContract::new(
                    "cms.pages",
                    10,
                    "Creates CMS page storage",
                )])
                .with_route_surfaces(vec![RouteSurface::new(
                    "cms.page",
                    davenda_core::RouteSurfaceKind::FrontendPage,
                    "/pages/{slug}",
                )])
                .with_jobs(vec![JobContract::new(
                    "cms.publish-scheduled",
                    davenda_core::JobTriggerKind::Scheduled,
                    true,
                    "Publishes scheduled pages",
                )])
                .with_event_subscriptions(vec![EventSubscription::new(
                    "cms.page.publish-requested",
                    Some("cms.publish-scheduled"),
                    "Schedules future publication work",
                )])
                .with_admin_resources(vec![AdminResourceContribution::new(
                    "cms.pages",
                    "/admin/pages",
                    "Pages",
                    "Pages",
                    AdminNavigationSection::Content,
                    AdminContributionKind::ResourceIndex,
                    Capability::CmsPagePublish,
                )])
                .with_search_contributions(vec![SearchIndexContribution::new(
                    "search.cms.pages",
                    SearchDocumentKind::Page,
                    SearchVisibility::Public,
                    true,
                    vec![SearchFieldContribution::new(
                        "title",
                        "title",
                        SearchFieldRole::Title,
                        true,
                        true,
                    )],
                    vec![SearchInvalidationRule::new(
                        SearchInvalidationTrigger::Published,
                        "page published",
                    )],
                    SearchRebuildStrategy::OnInvalidate,
                )]),
            ModuleManifest::new("commerce")
                .with_optional_capabilities(vec![Capability::OrderRead])
                .with_capability_contracts(vec![CapabilityContract::optional(
                    Capability::OrderRead,
                    ["order"],
                )])
                .with_module_dependencies(vec![davenda_core::ModuleDependency::required(
                    "cms",
                    "Commerce storefront installs depend on CMS navigation and content surfaces",
                )])
                .with_core_service_dependencies(vec![CoreServiceDependency::Jobs])
                .with_report_definitions(vec![ReportDefinition::new(
                    "report.orders.summary",
                    "Orders summary",
                    Some("Operational order summary".to_string()),
                    Capability::OrderRead,
                    ReportFormat::Csv,
                    ReportSensitivity::Restricted,
                    ReportDeliveryMode::InternalOnly,
                    "reports/orders",
                    davenda_jobs::RetryPolicy::new(
                        3,
                        std::time::Duration::from_secs(15),
                        std::time::Duration::from_secs(300),
                    )
                    .unwrap(),
                )])
                .with_bulk_operations(vec![BulkOperationDefinition::new(
                    "bulk.orders.export",
                    "Bulk export orders",
                    Some("Queues order exports".to_string()),
                    Capability::OrderRead,
                    BulkOperationKind::Export,
                    BulkOperationScope::Commerce,
                    davenda_jobs::RetryPolicy::new(
                        3,
                        std::time::Duration::from_secs(15),
                        std::time::Duration::from_secs(300),
                    )
                    .unwrap(),
                    Some(100),
                    true,
                )]),
        ]
    }

    #[test]
    fn manifest_requires_supported_default_locale_and_canonical_domain() {
        let invalid = CustomerAppManifest::new(
            CustomerAppId::new("invalid").unwrap(),
            "Invalid",
            locale("en-GB"),
            vec![locale("fr-FR")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("preview.example.com", false).unwrap());

        assert_eq!(
            invalid.validate().unwrap_err(),
            AppModelError::DefaultLocaleNotSupported {
                default_locale: "en-GB".to_string(),
            }
        );
    }

    #[test]
    fn manifest_rejects_duplicate_modules_and_extension_app_mismatch() {
        let duplicated = app().with_module(InstalledModuleSpec::new("cms").unwrap());
        assert_eq!(
            duplicated.validate().unwrap_err(),
            AppModelError::DuplicateInstalledModule {
                module: "cms".to_string(),
            }
        );

        let mismatched = CustomerAppManifest::new(
            CustomerAppId::new("mismatch").unwrap(),
            "Mismatch",
            locale("en-GB"),
            vec![locale("en-GB")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("mismatch.example.com", true).unwrap())
        .with_extension(
            CustomerExtension::new(
                "widget",
                ExtensionInstallation::new("other-app", Vec::new()).unwrap(),
            )
            .unwrap(),
        );
        assert_eq!(
            mismatched.validate().unwrap_err(),
            AppModelError::ExtensionCustomerAppMismatch {
                extension_id: "widget".to_string(),
                extension_customer_app: "other-app".to_string(),
                app_id: "mismatch".to_string(),
            }
        );
    }

    #[test]
    fn composition_collects_installed_module_contracts() {
        let composition = app()
            .compose(&DefaultAuthModelPackage::default(), &module_manifests())
            .unwrap();

        assert_eq!(composition.installed_modules.len(), 2);
        assert_eq!(composition.route_surfaces.len(), 1);
        assert_eq!(composition.jobs.len(), 1);
        assert_eq!(composition.event_subscriptions.len(), 1);
        assert_eq!(composition.admin_resources.len(), 1);
        assert_eq!(composition.search_contributions.len(), 1);
        assert_eq!(composition.report_definitions.len(), 1);
        assert_eq!(composition.bulk_operations.len(), 1);
        assert_eq!(composition.migrations.len(), 2);
        assert!(
            composition
                .required_core_services
                .contains(&CoreServiceDependency::Seo)
        );
        assert!(
            composition
                .required_core_services
                .contains(&CoreServiceDependency::Jobs)
        );
    }

    #[test]
    fn composition_rejects_unknown_modules_and_missing_dependencies() {
        let unknown = CustomerAppManifest::new(
            CustomerAppId::new("unknown").unwrap(),
            "Unknown",
            locale("en-GB"),
            vec![locale("en-GB")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("unknown.example.com", true).unwrap())
        .with_module(InstalledModuleSpec::new("events").unwrap());

        assert_eq!(
            unknown
                .compose(&DefaultAuthModelPackage::default(), &module_manifests())
                .unwrap_err(),
            AppModelError::UnknownInstalledModule {
                app_id: "unknown".to_string(),
                module: "events".to_string(),
            }
        );

        let missing_dependency = CustomerAppManifest::new(
            CustomerAppId::new("dependency").unwrap(),
            "Dependency",
            locale("en-GB"),
            vec![locale("en-GB")],
            theme(),
            auth(),
        )
        .unwrap()
        .with_domain(AppDomain::new("dependency.example.com", true).unwrap())
        .with_module(InstalledModuleSpec::new("commerce").unwrap());

        assert_eq!(
            missing_dependency
                .compose(&DefaultAuthModelPackage::default(), &module_manifests())
                .unwrap_err(),
            AppModelError::MissingModuleDependency {
                module: "commerce".to_string(),
                dependency: "cms".to_string(),
            }
        );
    }
}
