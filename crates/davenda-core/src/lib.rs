use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use davenda_a11y::{NavigationContract, ThemeAccessibilityContract};
use davenda_auth::{AuthModelPackage, Capability};
use davenda_cache::{CachePlanner, CacheTopology, DistributedCacheBackend};
use davenda_cli::CliRuntime;
use davenda_config::{
    CookieConfig as HttpCookieConfig, CsrfConfig as HttpCsrfConfig, DistributedCache,
    PlatformConfig, SameSitePolicy, SessionStore as ConfigSessionStore, TlsMode,
};
use davenda_data::{DataRuntime, MigrationPlan};
use davenda_i18n::{
    CurrencyCode, LocaleContext, LocaleRouter, LocaleTag, LocaleUrlConfig, TimeZoneId,
    TranslationCatalog, TranslationRuntime,
};
use davenda_jobs::JobsRuntime;
use davenda_observability::{
    DependencyKind, DependencyStatus, HealthProbeKind, HealthReport, MaintenanceMode,
    ObservabilityRuntime,
};
use davenda_seo::HeadMetadata;
use davenda_template::{TemplateNamespace, TemplateRegistry, TemplateRuntime};
use davenda_tls::TlsRuntime;
use davenda_wasm::{ExtensionPointKind, ResourceLimits};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDescriptor {
    pub id: String,
    pub owner: ServiceOwner,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceOwner {
    Core,
    Module(String),
    CustomerApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheRuntimeServices {
    pub topology: CacheTopology,
    pub planner: CachePlanner,
}

impl CacheRuntimeServices {
    pub fn shared_invalidation_enabled(&self) -> bool {
        self.topology.supports_shared_invalidation()
    }

    pub fn distributed_backend(&self) -> Option<DistributedCacheBackend> {
        self.topology.l2()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmRuntimeServices {
    pub extension_directory: String,
    pub allow_network: bool,
    pub limits: WasmLimitsProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmLimitsProfile {
    pub page: ResourceLimits,
    pub api: ResourceLimits,
    pub job: ResourceLimits,
    pub scheduled_job: ResourceLimits,
    pub webhook: ResourceLimits,
    pub admin_widget: ResourceLimits,
    pub render_hook: ResourceLimits,
}

impl WasmLimitsProfile {
    pub fn for_point(&self, point: ExtensionPointKind) -> ResourceLimits {
        match point {
            ExtensionPointKind::Page => self.page,
            ExtensionPointKind::Api => self.api,
            ExtensionPointKind::Job => self.job,
            ExtensionPointKind::ScheduledJob => self.scheduled_job,
            ExtensionPointKind::Webhook => self.webhook,
            ExtensionPointKind::AdminWidget => self.admin_widget,
            ExtensionPointKind::RenderHook => self.render_hook,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStoreTopology {
    Memory,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieProtection {
    Signed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookiePolicy {
    pub name: String,
    pub domain: Option<String>,
    pub path: String,
    pub same_site: SameSitePolicy,
    pub secure: bool,
    pub http_only: bool,
    pub protection: CookieProtection,
}

impl CookiePolicy {
    pub fn from_config(config: &HttpCookieConfig, protection: CookieProtection) -> Self {
        Self {
            name: config.name.clone(),
            domain: config.domain.clone(),
            path: config.path.clone(),
            same_site: config.same_site,
            secure: config.secure,
            http_only: config.http_only,
            protection,
        }
    }

    pub fn render_set_cookie(&self, value: &str, max_age: Option<Duration>) -> String {
        let mut attributes = vec![format!("{}={value}", self.name)];
        attributes.push(format!("Path={}", self.path));

        if let Some(domain) = &self.domain {
            attributes.push(format!("Domain={domain}"));
        }

        if let Some(max_age) = max_age {
            attributes.push(format!("Max-Age={}", max_age.as_secs()));
        }

        attributes.push(format!(
            "SameSite={}",
            match self.same_site {
                SameSitePolicy::Lax => "Lax",
                SameSitePolicy::Strict => "Strict",
                SameSitePolicy::None => "None",
            }
        ));

        if self.secure {
            attributes.push("Secure".to_string());
        }

        if self.http_only {
            attributes.push("HttpOnly".to_string());
        }

        attributes.join("; ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieSigner {
    pub policy: CookiePolicy,
}

impl CookieSigner {
    pub fn new(policy: CookiePolicy) -> Self {
        Self { policy }
    }

    pub fn sign(&self, secret: &[u8], value: &str) -> Result<String, BrowserSecurityError> {
        let payload = URL_SAFE_NO_PAD.encode(value.as_bytes());
        let signature = sign_payload(secret, payload.as_bytes())?;
        Ok(format!("v1.{payload}.{signature}"))
    }

    pub fn verify(&self, secret: &[u8], encoded: &str) -> Result<String, BrowserSecurityError> {
        let mut parts = encoded.split('.');
        let version = parts.next();
        let payload = parts.next();
        let signature = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidCookieFormat);
        }

        let payload = payload.ok_or(BrowserSecurityError::InvalidCookieFormat)?;
        let signature = signature.ok_or(BrowserSecurityError::InvalidCookieFormat)?;
        verify_payload(secret, payload.as_bytes(), signature)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| BrowserSecurityError::InvalidCookieFormat)?;

        String::from_utf8(bytes).map_err(|_| BrowserSecurityError::InvalidCookieFormat)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSecurityServices {
    pub store: SessionStoreTopology,
    pub idle_timeout: Duration,
    pub absolute_timeout: Duration,
    pub session_cookie: CookiePolicy,
    pub flash_cookie: CookiePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsrfProtection {
    pub enabled: bool,
    pub field_name: String,
    pub header_name: String,
}

impl CsrfProtection {
    pub fn from_config(config: &HttpCsrfConfig) -> Self {
        Self {
            enabled: config.enabled,
            field_name: config.field_name.clone(),
            header_name: config.header_name.clone(),
        }
    }

    pub fn issue_token(
        &self,
        secret: &[u8],
        session_id: &str,
        action: &str,
    ) -> Result<String, BrowserSecurityError> {
        if !self.enabled {
            return Err(BrowserSecurityError::CsrfDisabled);
        }

        let mut nonce = [0u8; 16];
        OsRng.fill_bytes(&mut nonce);
        let nonce = URL_SAFE_NO_PAD.encode(nonce);
        let payload = format!("{session_id}:{action}:{nonce}");
        let signature = sign_payload(secret, payload.as_bytes())?;
        Ok(format!("v1.{nonce}.{signature}"))
    }

    pub fn verify_token(
        &self,
        secret: &[u8],
        session_id: &str,
        action: &str,
        token: &str,
    ) -> Result<bool, BrowserSecurityError> {
        if !self.enabled {
            return Ok(true);
        }

        let mut parts = token.split('.');
        let version = parts.next();
        let nonce = parts.next();
        let signature = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidCsrfTokenFormat);
        }

        let nonce = nonce.ok_or(BrowserSecurityError::InvalidCsrfTokenFormat)?;
        let signature = signature.ok_or(BrowserSecurityError::InvalidCsrfTokenFormat)?;
        let payload = format!("{session_id}:{action}:{nonce}");

        Ok(verify_payload(secret, payload.as_bytes(), signature).is_ok())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSecurityServices {
    pub sessions: SessionSecurityServices,
    pub csrf: CsrfProtection,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrowserSecurityError {
    #[error("browser security operations require a non-empty secret")]
    EmptySecret,
    #[error("cookie value is not in the expected signed format")]
    InvalidCookieFormat,
    #[error("signed cookie failed verification")]
    InvalidCookieSignature,
    #[error("CSRF protection is disabled for this runtime")]
    CsrfDisabled,
    #[error("CSRF token is not in the expected format")]
    InvalidCsrfTokenFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRuntimeServices {
    pub customer_app_namespace: TemplateNamespace,
    pub core_namespace: TemplateNamespace,
    pub registry: TemplateRegistry,
    pub runtime: TemplateRuntime,
}

impl TemplateRuntimeServices {
    pub fn namespace_chain(
        &self,
        module_namespace: Option<&TemplateNamespace>,
    ) -> Vec<TemplateNamespace> {
        let mut chain = vec![self.customer_app_namespace.clone()];

        if let Some(module_namespace) = module_namespace {
            if module_namespace != &self.customer_app_namespace
                && module_namespace != &self.core_namespace
            {
                chain.push(module_namespace.clone());
            }
        }

        chain.push(self.core_namespace.clone());
        chain
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct I18nRuntimeServices {
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub fallback_locale: LocaleTag,
    pub router: LocaleRouter,
    pub translations: TranslationRuntime,
}

impl I18nRuntimeServices {
    pub fn request_context(&self, requested_locale: Option<&str>) -> LocaleContext {
        let resolved = requested_locale
            .and_then(|locale| {
                self.supported_locales
                    .iter()
                    .find(|candidate| candidate.as_str() == locale)
            })
            .cloned()
            .unwrap_or_else(|| self.default_locale.clone());

        LocaleContext::new(
            resolved.clone(),
            vec![self.fallback_locale.clone()],
            currency_for_locale(&resolved),
            timezone_for_locale(&resolved),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoRuntimeServices {
    pub canonical_host: String,
    pub emit_json_ld: bool,
    pub sitemap_enabled: bool,
}

impl SeoRuntimeServices {
    pub fn allows_json_ld(&self) -> bool {
        self.emit_json_ld
    }

    pub fn can_emit_metadata(&self, metadata: &HeadMetadata) -> bool {
        !metadata.canonical_url.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct A11yRuntimeServices {
    pub navigation: NavigationContract,
    pub theme_baseline: ThemeAccessibilityContract,
}

pub type CliRuntimeServices = CliRuntime;
pub type DataRuntimeServices = DataRuntime;
pub type JobsRuntimeServices = JobsRuntime;
pub type ObservabilityRuntimeServices = ObservabilityRuntime;
pub type TlsRuntimeServices = TlsRuntime;

#[derive(Debug, Clone)]
pub struct CoreBootstrap {
    pub registry: ServiceRegistry,
    pub cache: CacheRuntimeServices,
    pub browser: BrowserSecurityServices,
    pub cli: CliRuntimeServices,
    pub data: DataRuntimeServices,
    pub jobs: JobsRuntimeServices,
    pub observability: ObservabilityRuntimeServices,
    pub i18n: I18nRuntimeServices,
    pub seo: SeoRuntimeServices,
    pub a11y: A11yRuntimeServices,
    pub template: TemplateRuntimeServices,
    pub tls: TlsRuntimeServices,
    pub wasm: WasmRuntimeServices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleManifest {
    pub name: String,
    pub required_capabilities: Vec<Capability>,
    pub optional_capabilities: Vec<Capability>,
    pub config_namespace: Option<String>,
    pub capability_contracts: Vec<CapabilityContract>,
    pub module_dependencies: Vec<ModuleDependency>,
    pub core_service_dependencies: Vec<CoreServiceDependency>,
    pub migrations: Vec<MigrationContract>,
    pub route_surfaces: Vec<RouteSurface>,
    pub jobs: Vec<JobContract>,
    pub event_subscriptions: Vec<EventSubscription>,
    pub integration_points: Vec<IntegrationPoint>,
    pub behaviors: Vec<ModuleBehavior>,
    pub extension_slots: Vec<ExtensionSlotDescriptor>,
    pub admin_resources: Vec<AdminResourceContribution>,
    pub http_surfaces: Vec<HttpSurfaceContribution>,
}

impl ModuleManifest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required_capabilities: Vec::new(),
            optional_capabilities: Vec::new(),
            config_namespace: None,
            capability_contracts: Vec::new(),
            module_dependencies: Vec::new(),
            core_service_dependencies: Vec::new(),
            migrations: Vec::new(),
            route_surfaces: Vec::new(),
            jobs: Vec::new(),
            event_subscriptions: Vec::new(),
            integration_points: Vec::new(),
            behaviors: Vec::new(),
            extension_slots: Vec::new(),
            admin_resources: Vec::new(),
            http_surfaces: Vec::new(),
        }
    }

    pub fn with_required_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.required_capabilities = capabilities;
        self
    }

    pub fn with_optional_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.optional_capabilities = capabilities;
        self
    }

    pub fn with_config_namespace(mut self, config_namespace: impl Into<String>) -> Self {
        self.config_namespace = Some(config_namespace.into());
        self
    }

    pub fn with_capability_contracts(mut self, contracts: Vec<CapabilityContract>) -> Self {
        self.capability_contracts = contracts;
        self
    }

    pub fn with_module_dependencies(mut self, dependencies: Vec<ModuleDependency>) -> Self {
        self.module_dependencies = dependencies;
        self
    }

    pub fn with_core_service_dependencies(
        mut self,
        dependencies: Vec<CoreServiceDependency>,
    ) -> Self {
        self.core_service_dependencies = dependencies;
        self
    }

    pub fn with_migrations(mut self, migrations: Vec<MigrationContract>) -> Self {
        self.migrations = migrations;
        self
    }

    pub fn with_route_surfaces(mut self, routes: Vec<RouteSurface>) -> Self {
        self.route_surfaces = routes;
        self
    }

    pub fn with_jobs(mut self, jobs: Vec<JobContract>) -> Self {
        self.jobs = jobs;
        self
    }

    pub fn with_event_subscriptions(
        mut self,
        subscriptions: Vec<EventSubscription>,
    ) -> Self {
        self.event_subscriptions = subscriptions;
        self
    }

    pub fn with_integration_points(mut self, integrations: Vec<IntegrationPoint>) -> Self {
        self.integration_points = integrations;
        self
    }

    pub fn with_behaviors(mut self, behaviors: Vec<ModuleBehavior>) -> Self {
        self.behaviors = behaviors;
        self
    }

    pub fn with_extension_slots(mut self, extension_slots: Vec<ExtensionSlotDescriptor>) -> Self {
        self.extension_slots = extension_slots;
        self
    }

    pub fn with_admin_resources(mut self, admin_resources: Vec<AdminResourceContribution>) -> Self {
        self.admin_resources = admin_resources;
        self
    }

    pub fn with_http_surfaces(mut self, http_surfaces: Vec<HttpSurfaceContribution>) -> Self {
        self.http_surfaces = http_surfaces;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityContract {
    pub capability: Capability,
    pub required: bool,
    pub resource_kinds: Vec<String>,
}

impl CapabilityContract {
    pub fn required(
        capability: Capability,
        resource_kinds: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::new(capability, true, resource_kinds)
    }

    pub fn optional(
        capability: Capability,
        resource_kinds: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self::new(capability, false, resource_kinds)
    }

    fn new(
        capability: Capability,
        required: bool,
        resource_kinds: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            capability,
            required,
            resource_kinds: resource_kinds.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleDependencyKind {
    Required,
    Optional,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDependency {
    pub module: String,
    pub kind: ModuleDependencyKind,
    pub reason: String,
}

impl ModuleDependency {
    pub fn required(
        module: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            module: module.into(),
            kind: ModuleDependencyKind::Required,
            reason: reason.into(),
        }
    }

    pub fn optional(
        module: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            module: module.into(),
            kind: ModuleDependencyKind::Optional,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreServiceDependency {
    Auth,
    Data,
    Cache,
    Jobs,
    Storage,
    Assets,
    I18n,
    Seo,
    A11y,
    Template,
    Wasm,
    Observability,
    BrowserSecurity,
    Http,
    Tls,
}

impl CoreServiceDependency {
    pub fn required_service_ids(self) -> &'static [&'static str] {
        match self {
            Self::Auth => &["core.auth"],
            Self::Data => &["core.data", "core.data.migrations"],
            Self::Cache => &["core.cache.l1", "core.cache.http"],
            Self::Jobs => &["core.jobs"],
            Self::Storage => &["core.storage"],
            Self::Assets => &["core.assets"],
            Self::I18n => &["core.i18n"],
            Self::Seo => &["core.seo"],
            Self::A11y => &["core.a11y"],
            Self::Template => &["core.template", "core.template.fragments"],
            Self::Wasm => &["core.wasm", "core.wasm.limits"],
            Self::Observability => &["core.health", "core.maintenance", "core.flags"],
            Self::BrowserSecurity => {
                &["core.http.sessions", "core.http.cookies", "core.http.csrf"]
            }
            Self::Http => &["core.http"],
            Self::Tls => &["core.tls.reload"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationContract {
    pub owner: String,
    pub order: u32,
    pub description: String,
}

impl MigrationContract {
    pub fn new(
        owner: impl Into<String>,
        order: u32,
        description: impl Into<String>,
    ) -> Self {
        Self {
            owner: owner.into(),
            order,
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSurfaceKind {
    FrontendPage,
    FrontendAction,
    AdminPage,
    AdminAction,
    Api,
    Fragment,
    Asset,
    Webhook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSurface {
    pub name: String,
    pub kind: RouteSurfaceKind,
    pub path: String,
    pub localized: bool,
    pub capability: Option<Capability>,
}

impl RouteSurface {
    pub fn new(
        name: impl Into<String>,
        kind: RouteSurfaceKind,
        path: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            path: path.into(),
            localized: false,
            capability: None,
        }
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn gated_by(mut self, capability: Capability) -> Self {
        self.capability = Some(capability);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobTriggerKind {
    Scheduled,
    DomainEvent,
    Operator,
    Webhook,
    InlineFollowup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobContract {
    pub name: String,
    pub trigger: JobTriggerKind,
    pub idempotent: bool,
    pub description: String,
}

impl JobContract {
    pub fn new(
        name: impl Into<String>,
        trigger: JobTriggerKind,
        idempotent: bool,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            trigger,
            idempotent,
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventSubscription {
    pub event: String,
    pub job: Option<String>,
    pub description: String,
}

impl EventSubscription {
    pub fn new(
        event: impl Into<String>,
        job: Option<impl Into<String>>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            event: event.into(),
            job: job.map(Into::into),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationKind {
    AdminNavigation,
    AdminWorkflow,
    FrontendRendering,
    SearchIndex,
    SeoMetadata,
    JsonLd,
    LocalizedContent,
    CacheInvalidation,
    StoragePolicy,
    CommerceBridge,
    AuthPublication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationPoint {
    pub kind: IntegrationKind,
    pub surface: String,
    pub description: String,
}

impl IntegrationPoint {
    pub fn new(
        kind: IntegrationKind,
        surface: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            surface: surface.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleBehavior {
    CacheInvalidation,
    LocalizedContent,
    SeoMetadata,
    JsonLd,
    AccessibleAdminUi,
    StoragePolicyAware,
    AuthGovernedPublication,
    AsyncJobs,
    AuditedBulkActions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionSlotKind {
    Page,
    Api,
    Job,
    ScheduledJob,
    Webhook,
    AdminWidget,
    RenderHook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSlotDescriptor {
    pub kind: ExtensionSlotKind,
    pub surface: String,
    pub description: String,
}

impl ExtensionSlotDescriptor {
    pub fn new(
        kind: ExtensionSlotKind,
        surface: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            surface: surface.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminNavigationSection {
    Overview,
    Content,
    Commerce,
    Memberships,
    Events,
    Media,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminContributionKind {
    Dashboard,
    ResourceIndex,
    DetailView,
    Workflow,
    Audit,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResourceContribution {
    pub id: String,
    pub route: String,
    pub title: String,
    pub nav_label: String,
    pub section: AdminNavigationSection,
    pub kind: AdminContributionKind,
    pub required_capability: Capability,
}

impl AdminResourceContribution {
    pub fn new(
        id: impl Into<String>,
        route: impl Into<String>,
        title: impl Into<String>,
        nav_label: impl Into<String>,
        section: AdminNavigationSection,
        kind: AdminContributionKind,
        required_capability: Capability,
    ) -> Self {
        Self {
            id: id.into(),
            route: route.into(),
            title: title.into(),
            nav_label: nav_label.into(),
            section,
            kind,
            required_capability,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpSurfaceArea {
    Public,
    Account,
    Admin,
    Api,
    Fragment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpSurfaceMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpFileDeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpResponseContract {
    Page {
        template: String,
        status: u16,
    },
    Fragment {
        template: String,
        fragment_id: String,
    },
    Redirect {
        location: String,
        status: u16,
    },
    Json {
        status: u16,
        payload: BTreeMap<String, String>,
    },
    File {
        logical_path: String,
        content_type: String,
        delivery_mode: HttpFileDeliveryMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSurfaceContribution {
    pub name: String,
    pub method: HttpSurfaceMethod,
    pub path: String,
    pub area: HttpSurfaceArea,
    pub localized: bool,
    pub capability: Option<Capability>,
    pub response: HttpResponseContract,
}

impl HttpSurfaceContribution {
    pub fn page(
        name: impl Into<String>,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            method: HttpSurfaceMethod::Get,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::Page {
                template: template.into(),
                status: 200,
            },
        }
    }

    pub fn fragment(
        name: impl Into<String>,
        path: impl Into<String>,
        template: impl Into<String>,
        fragment_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            method: HttpSurfaceMethod::Get,
            path: path.into(),
            area: HttpSurfaceArea::Fragment,
            localized: false,
            capability: None,
            response: HttpResponseContract::Fragment {
                template: template.into(),
                fragment_id: fragment_id.into(),
            },
        }
    }

    pub fn json(
        name: impl Into<String>,
        method: HttpSurfaceMethod,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        status: u16,
        payload: BTreeMap<String, String>,
    ) -> Self {
        Self {
            name: name.into(),
            method,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::Json { status, payload },
        }
    }

    pub fn redirect(
        name: impl Into<String>,
        method: HttpSurfaceMethod,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        location: impl Into<String>,
        status: u16,
    ) -> Self {
        Self {
            name: name.into(),
            method,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::Redirect {
                location: location.into(),
                status,
            },
        }
    }

    pub fn file(
        name: impl Into<String>,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: HttpFileDeliveryMode,
    ) -> Self {
        Self {
            name: name.into(),
            method: HttpSurfaceMethod::Get,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::File {
                logical_path: logical_path.into(),
                content_type: content_type.into(),
                delivery_mode,
            },
        }
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn gated_by(mut self, capability: Capability) -> Self {
        self.capability = Some(capability);
        self
    }
}

pub trait PlatformModule {
    fn manifest(&self) -> ModuleManifest;
    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError>;
    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        None
    }
}

#[derive(Debug, Default, Clone)]
pub struct ServiceRegistry {
    services: HashMap<String, ServiceDescriptor>,
    modules: HashMap<String, ModuleManifest>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_core_service(
        &mut self,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceDescriptor {
            id: id.into(),
            owner: ServiceOwner::Core,
            description: description.into(),
        })
    }

    pub fn register_module_service(
        &mut self,
        module: impl Into<String>,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceDescriptor {
            id: id.into(),
            owner: ServiceOwner::Module(module.into()),
            description: description.into(),
        })
    }

    pub fn register_customer_app_service(
        &mut self,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceDescriptor {
            id: id.into(),
            owner: ServiceOwner::CustomerApp,
            description: description.into(),
        })
    }

    pub fn register_module_manifest(
        &mut self,
        manifest: ModuleManifest,
    ) -> Result<(), RegistrationError> {
        if self.modules.contains_key(&manifest.name) {
            return Err(RegistrationError::DuplicateModule {
                name: manifest.name.clone(),
            });
        }

        self.modules.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    pub fn services(&self) -> impl Iterator<Item = &ServiceDescriptor> {
        self.services.values()
    }

    pub fn modules(&self) -> impl Iterator<Item = &ModuleManifest> {
        self.modules.values()
    }

    fn register(&mut self, service: ServiceDescriptor) -> Result<(), RegistrationError> {
        if self.services.contains_key(&service.id) {
            return Err(RegistrationError::DuplicateService {
                id: service.id.clone(),
            });
        }

        self.services.insert(service.id.clone(), service);
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistrationError {
    #[error("service `{id}` is already registered")]
    DuplicateService { id: String },
    #[error("module `{name}` is already registered")]
    DuplicateModule { name: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityValidationError {
    #[error(
        "module `{module}` requires capability `{capability}` but the active auth package does not bind it"
    )]
    MissingCapability {
        module: String,
        capability: Capability,
    },
    #[error(
        "module `{module}` does not declare a capability contract for `{capability}`"
    )]
    MissingCapabilityContract {
        module: String,
        capability: Capability,
    },
    #[error(
        "module `{module}` declares capability `{capability}` as {actual} but {expected} was required"
    )]
    CapabilityContractRoleMismatch {
        module: String,
        capability: Capability,
        expected: &'static str,
        actual: &'static str,
    },
    #[error(
        "module `{module}` declares capability `{capability}` without any resource kinds"
    )]
    EmptyCapabilityResourceKinds {
        module: String,
        capability: Capability,
    },
    #[error(
        "module `{module}` declares a capability contract for `{capability}` without listing it as required or optional"
    )]
    UndeclaredCapabilityContract {
        module: String,
        capability: Capability,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleInstallationError {
    #[error("module `{module}` requires module dependency `{dependency}`")]
    MissingModuleDependency {
        module: String,
        dependency: String,
    },
    #[error(
        "module `{module}` requires core dependency `{dependency:?}` but service `{service_id}` is not available"
    )]
    MissingCoreServiceDependency {
        module: String,
        dependency: CoreServiceDependency,
        service_id: String,
    },
}

pub fn bootstrap_core_services(
    config: &PlatformConfig,
) -> Result<CoreBootstrap, RegistrationError> {
    let mut registry = ServiceRegistry::new();
    let cache_topology = cache_topology_from_config(config);
    let cache = CacheRuntimeServices {
        topology: cache_topology,
        planner: CachePlanner::new(cache_topology),
    };
    let browser = browser_security_from_config(config);
    let cli = cli_runtime_from_config(config);
    let data = data_runtime_from_config(config);
    let jobs = jobs_runtime_from_config(config);
    let observability = observability_runtime_from_config(config);
    let i18n = i18n_runtime_from_config(config);
    let seo = seo_runtime_from_config(config);
    let a11y = a11y_runtime_services();
    let template = template_runtime_services();
    let tls = tls_runtime_from_config(config);
    let wasm = wasm_runtime_from_config(config);

    registry.register_core_service("core.config", "Typed platform configuration")?;
    registry.register_core_service(
        "core.cli",
        format!(
            "Platform CLI contract with {} baseline commands",
            cli.registry.commands().count()
        ),
    )?;
    registry.register_core_service("core.logging", "Structured logging service")?;
    registry.register_core_service(
        "core.health",
        "Liveness, readiness, and operator-facing dependency health checks",
    )?;
    registry.register_core_service(
        "core.maintenance",
        "Maintenance-mode control for deployment-wide and customer-app-scoped traffic shaping",
    )?;
    registry.register_core_service(
        "core.flags",
        "Scoped feature-flag control plane for staged rollout and customer targeting",
    )?;

    if config.observability.metrics {
        registry.register_core_service(
            "core.metrics",
            "Structured metric catalog for HTTP, auth, cache, queue, TLS, storage, and extensions",
        )?;
    }

    if config.observability.tracing {
        registry.register_core_service("core.tracing", "Distributed tracing pipeline")?;
    }

    registry.register_core_service("core.auth", "Authorization engine and model loader")?;
    registry.register_core_service(
        "core.data",
        format!(
            "Primary {:?} data access with schema `{}` and pool {}..{}",
            data.driver, data.schema, data.pool.min_connections, data.pool.max_connections
        ),
    )?;
    registry.register_core_service(
        "core.data.migrations",
        format!(
            "Owned migration planning through `{}`",
            data.migrations_table
        ),
    )?;
    registry.register_core_service(
        "core.cache.l1",
        format!("Local cache backend: {}", cache.topology.l1()),
    )?;

    if let Some(distributed) = cache.distributed_backend() {
        registry.register_core_service(
            "core.cache.l2",
            format!("Distributed cache backend: {distributed}"),
        )?;
        registry.register_core_service(
            "core.cache.invalidation",
            format!("Shared invalidation, coalescing, and coordination via {distributed}"),
        )?;
    }
    registry.register_core_service(
        "core.cache.http",
        "HTTP cache-control, validators, variation keys, and surrogate tags",
    )?;
    registry.register_core_service(
        "core.http",
        "HTTP request pipeline, middleware ordering, and typed request context",
    )?;
    registry.register_core_service(
        "core.http.sessions",
        format!(
            "Server-side session policy backed by {:?}",
            browser.sessions.store
        ),
    )?;
    registry.register_core_service(
        "core.http.cookies",
        "Signed cookie policy with central Secure, HttpOnly, SameSite, domain, and path defaults",
    )?;
    registry.register_core_service(
        "core.http.csrf",
        "CSRF token issuance and validation for state-changing browser flows",
    )?;

    registry.register_core_service("core.storage", "Storage policy and object access layer")?;
    registry.register_core_service("core.assets", "Asset manifest and CDN publication layer")?;
    registry.register_core_service(
        "core.i18n",
        format!(
            "Locale resolution, fallback translation runtime, and URL generation rooted at `{}`",
            seo.canonical_host
        ),
    )?;
    registry.register_core_service(
        "core.seo",
        format!(
            "Typed metadata, sitemap, canonical URL, and JSON-LD services with sitemap {}",
            if seo.sitemap_enabled {
                "enabled"
            } else {
                "disabled"
            }
        ),
    )?;
    registry.register_core_service(
        "core.a11y",
        "Accessibility-aware form, table, dialog, navigation, live-region, and theme-baseline contracts",
    )?;
    registry.register_core_service("core.template", "HTML-first template runtime")?;
    registry.register_core_service(
        "core.template.fragments",
        "Named fragment, slot, and partial-rendering composition runtime",
    )?;
    registry.register_core_service(
        "core.wasm",
        format!(
            "WASM extension host runtime rooted at `{}` with network {}",
            wasm.extension_directory,
            if wasm.allow_network {
                "enabled"
            } else {
                "disabled"
            }
        ),
    )?;
    registry.register_core_service(
        "core.wasm.limits",
        "Per-surface WASM resource limits for pages, APIs, jobs, webhooks, and widgets",
    )?;
    registry.register_core_service(
        "core.jobs",
        format!(
            "Background jobs, scheduler, and domain-event queues over {:?}",
            jobs.backend
        ),
    )?;

    match tls.mode {
        TlsMode::External => {
            registry.register_core_service(
                "core.tls.metadata",
                "Trusted termination metadata and secure transport policy",
            )?;
        }
        _ => {
            registry.register_core_service(
                "core.tls",
                "Certificate lifecycle, TLS termination, and renewal orchestration",
            )?;
            registry.register_core_service(
                "core.tls.reload",
                "Hot-reloadable certificate bindings and SNI inventory",
            )?;
        }
    }

    Ok(CoreBootstrap {
        registry,
        cache,
        browser,
        cli,
        data,
        jobs,
        observability,
        i18n,
        seo,
        a11y,
        template,
        tls,
        wasm,
    })
}

pub fn validate_module_capabilities<P>(
    package: &P,
    manifest: &ModuleManifest,
) -> Result<(), CapabilityValidationError>
where
    P: AuthModelPackage,
{
    for capability in &manifest.required_capabilities {
        if package.binding_for(*capability).is_none() {
            return Err(CapabilityValidationError::MissingCapability {
                module: manifest.name.clone(),
                capability: *capability,
            });
        }

        validate_capability_contract(manifest, *capability, true)?;
    }

    for capability in &manifest.optional_capabilities {
        validate_capability_contract(manifest, *capability, false)?;
    }

    for contract in &manifest.capability_contracts {
        let declared = manifest.required_capabilities.contains(&contract.capability)
            || manifest.optional_capabilities.contains(&contract.capability);
        if !declared {
            return Err(CapabilityValidationError::UndeclaredCapabilityContract {
                module: manifest.name.clone(),
                capability: contract.capability,
            });
        }
    }

    Ok(())
}

pub fn validate_module_installation(
    manifest: &ModuleManifest,
    installed_modules: &[String],
    core_service_ids: &[&str],
) -> Result<(), ModuleInstallationError> {
    for dependency in &manifest.module_dependencies {
        if dependency.kind == ModuleDependencyKind::Required
            && !installed_modules.contains(&dependency.module)
        {
            return Err(ModuleInstallationError::MissingModuleDependency {
                module: manifest.name.clone(),
                dependency: dependency.module.clone(),
            });
        }
    }

    for dependency in &manifest.core_service_dependencies {
        for service_id in dependency.required_service_ids() {
            if !core_service_ids.contains(service_id) {
                return Err(ModuleInstallationError::MissingCoreServiceDependency {
                    module: manifest.name.clone(),
                    dependency: *dependency,
                    service_id: (*service_id).to_string(),
                });
            }
        }
    }

    Ok(())
}

fn validate_capability_contract(
    manifest: &ModuleManifest,
    capability: Capability,
    required: bool,
) -> Result<(), CapabilityValidationError> {
    let Some(contract) = manifest
        .capability_contracts
        .iter()
        .find(|contract| contract.capability == capability)
    else {
        return Err(CapabilityValidationError::MissingCapabilityContract {
            module: manifest.name.clone(),
            capability,
        });
    };

    if contract.required != required {
        return Err(CapabilityValidationError::CapabilityContractRoleMismatch {
            module: manifest.name.clone(),
            capability,
            expected: if required { "required" } else { "optional" },
            actual: if contract.required {
                "required"
            } else {
                "optional"
            },
        });
    }

    if contract.resource_kinds.is_empty() {
        return Err(CapabilityValidationError::EmptyCapabilityResourceKinds {
            module: manifest.name.clone(),
            capability,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_cache::DistributedCacheBackend;
    use davenda_config::PlatformConfig;
    use davenda_template::TemplateNamespace;
    use davenda_wasm::ExtensionPointKind;

    const VALID_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
object_store = "s3"
local_root = "/var/lib/platform"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false

[modules]
enabled = ["cms-pages", "admin-shell"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#;

    #[test]
    fn bootstrap_registers_core_services() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let bootstrap = bootstrap_core_services(&config).unwrap();

        let ids = bootstrap
            .registry
            .services()
            .map(|service| service.id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"core.config"));
        assert!(ids.contains(&"core.cli"));
        assert!(ids.contains(&"core.auth"));
        assert!(ids.contains(&"core.tls"));
        assert!(ids.contains(&"core.tls.reload"));
        assert!(ids.contains(&"core.data"));
        assert!(ids.contains(&"core.data.migrations"));
        assert!(ids.contains(&"core.jobs"));
        assert!(ids.contains(&"core.health"));
        assert!(ids.contains(&"core.maintenance"));
        assert!(ids.contains(&"core.flags"));
        assert!(ids.contains(&"core.metrics"));
        assert!(ids.contains(&"core.cache.l1"));
        assert!(ids.contains(&"core.cache.l2"));
        assert!(ids.contains(&"core.cache.invalidation"));
        assert!(ids.contains(&"core.cache.http"));
        assert!(ids.contains(&"core.http"));
        assert!(ids.contains(&"core.http.sessions"));
        assert!(ids.contains(&"core.http.cookies"));
        assert!(ids.contains(&"core.http.csrf"));
        assert!(ids.contains(&"core.i18n"));
        assert!(ids.contains(&"core.seo"));
        assert!(ids.contains(&"core.a11y"));
        assert!(ids.contains(&"core.template.fragments"));
        assert!(ids.contains(&"core.wasm"));
        assert!(ids.contains(&"core.wasm.limits"));
        assert_eq!(
            bootstrap.cache.distributed_backend(),
            Some(DistributedCacheBackend::Redis)
        );
        assert!(bootstrap.cache.shared_invalidation_enabled());
        assert_eq!(
            bootstrap.browser.sessions.store,
            SessionStoreTopology::Redis
        );
        assert_eq!(
            bootstrap.browser.sessions.idle_timeout,
            Duration::from_secs(3600)
        );
        assert_eq!(
            bootstrap.browser.sessions.session_cookie.name,
            "davenda_session"
        );
        assert_eq!(bootstrap.browser.csrf.field_name, "_csrf");
        assert!(
            bootstrap
                .cli
                .registry
                .commands()
                .any(|command| command.path == vec!["config".to_string(), "validate".to_string()])
        );
        assert_eq!(
            bootstrap.data.driver,
            davenda_config::DatabaseDriver::Postgres
        );
        assert_eq!(bootstrap.data.schema, "public");
        assert_eq!(bootstrap.data.migrations_table, "_davenda_migrations");
        assert_eq!(bootstrap.jobs.backend, davenda_config::JobBackend::Redis);
        assert_eq!(bootstrap.jobs.topology.work_queue.as_str(), "jobs.work");
        assert_eq!(
            bootstrap.jobs.topology.domain_events_queue.as_str(),
            "jobs.domain-events"
        );
        assert_eq!(bootstrap.tls.mode, TlsMode::Acme);
        assert_eq!(
            bootstrap.tls.provider,
            Some(davenda_tls::CertificateProviderKind::CloudflareDns)
        );
        assert_eq!(
            bootstrap.tls.challenge,
            Some(davenda_tls::ChallengeStrategy::Dns01)
        );
        assert!(bootstrap.tls.hot_reload_supported);
        assert!(bootstrap.observability.telemetry.metrics_enabled);
        assert!(bootstrap.observability.telemetry.trace.enabled);
        assert!(
            bootstrap
                .observability
                .readiness
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == DependencyKind::Database)
        );
        assert!(
            bootstrap
                .observability
                .readiness
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == DependencyKind::DistributedCache)
        );
        assert!(
            bootstrap
                .observability
                .readiness
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == DependencyKind::Queue)
        );
        assert!(
            bootstrap
                .observability
                .readiness
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == DependencyKind::ObjectStore)
        );
        assert!(
            bootstrap
                .observability
                .readiness
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == DependencyKind::Secrets)
        );
        assert!(
            bootstrap
                .observability
                .readiness
                .dependencies
                .iter()
                .any(|dependency| dependency.kind == DependencyKind::Tls)
        );
        let locale_context = bootstrap.i18n.request_context(Some("fr-FR"));
        assert_eq!(locale_context.locale.as_str(), "fr-FR");
        assert_eq!(locale_context.currency.as_str(), "EUR");
        assert_eq!(locale_context.timezone.as_str(), "Europe/Paris");
        assert_eq!(
            bootstrap
                .i18n
                .router
                .absolute_url(&bootstrap.i18n.default_locale, "/events")
                .unwrap(),
            "https://www.example.com/en-GB/events"
        );
        assert!(bootstrap.seo.emit_json_ld);
        assert!(bootstrap.seo.sitemap_enabled);
        assert_eq!(bootstrap.a11y.navigation.skip_link_target, "main-content");
        assert!(bootstrap.a11y.theme_baseline.meets_platform_baseline());
        assert_eq!(
            bootstrap
                .template
                .namespace_chain(Some(&TemplateNamespace::new("events").unwrap())),
            vec![
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateNamespace::new("events").unwrap(),
                TemplateNamespace::new("core").unwrap(),
            ]
        );
        assert_eq!(bootstrap.wasm.extension_directory, "extensions");
        assert!(!bootstrap.wasm.allow_network);
        assert_eq!(
            bootstrap
                .wasm
                .limits
                .for_point(ExtensionPointKind::Page)
                .max_runtime,
            Duration::from_millis(50)
        );
        assert_eq!(
            bootstrap
                .wasm
                .limits
                .for_point(ExtensionPointKind::Job)
                .max_runtime,
            Duration::from_secs(30)
        );
    }

    #[test]
    fn validates_module_capabilities_against_auth_package() {
        let package = DefaultAuthModelPackage::default();
        let manifest = ModuleManifest::new("cms-pages")
            .with_required_capabilities(vec![Capability::CmsPageRead, Capability::CmsPagePublish])
            .with_capability_contracts(vec![
                CapabilityContract::required(Capability::CmsPageRead, ["page"]),
                CapabilityContract::required(Capability::CmsPagePublish, ["page"]),
            ]);

        assert!(validate_module_capabilities(&package, &manifest).is_ok());
    }

    #[test]
    fn validates_module_installation_dependencies_against_installed_modules_and_core_services() {
        let manifest = ModuleManifest::new("memberships")
            .with_module_dependencies(vec![ModuleDependency::required(
                "commerce",
                "subscription purchases depend on order outcomes",
            )])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Jobs,
            ]);

        let missing_dependency = validate_module_installation(
            &manifest,
            &["cms".to_string()],
            &["core.auth", "core.data", "core.data.migrations", "core.jobs"],
        )
        .unwrap_err();
        assert_eq!(
            missing_dependency,
            ModuleInstallationError::MissingModuleDependency {
                module: "memberships".to_string(),
                dependency: "commerce".to_string(),
            }
        );

        assert!(validate_module_installation(
            &manifest,
            &["commerce".to_string(), "memberships".to_string()],
            &["core.auth", "core.data", "core.data.migrations", "core.jobs"],
        )
        .is_ok());
    }

    #[test]
    fn signed_cookie_round_trips_and_rejects_tampering() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let bootstrap = bootstrap_core_services(&config).unwrap();
        let signer = CookieSigner::new(bootstrap.browser.sessions.session_cookie.clone());
        let secret = b"0123456789abcdef0123456789abcdef";

        let signed = signer.sign(secret, "sess_123").unwrap();
        assert_eq!(signer.verify(secret, &signed).unwrap(), "sess_123");

        let mut tampered = signed.clone();
        let last = tampered.pop().unwrap();
        tampered.push(if last == 'A' { 'B' } else { 'A' });
        assert!(signer.verify(secret, &tampered).is_err());
    }

    #[test]
    fn csrf_tokens_bind_to_session_and_action() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let bootstrap = bootstrap_core_services(&config).unwrap();
        let secret = b"abcdef0123456789abcdef0123456789";

        let token = bootstrap
            .browser
            .csrf
            .issue_token(secret, "sess_123", "/checkout")
            .unwrap();

        assert!(
            bootstrap
                .browser
                .csrf
                .verify_token(secret, "sess_123", "/checkout", &token)
                .unwrap()
        );
        assert!(
            !bootstrap
                .browser
                .csrf
                .verify_token(secret, "sess_999", "/checkout", &token)
                .unwrap()
        );
    }
}

fn cache_topology_from_config(config: &PlatformConfig) -> CacheTopology {
    match config.cache.l2 {
        Some(DistributedCache::Redis) => CacheTopology::with_redis(),
        Some(DistributedCache::Valkey) => CacheTopology::with_valkey(),
        None => CacheTopology::moka_only(),
    }
}

fn browser_security_from_config(config: &PlatformConfig) -> BrowserSecurityServices {
    BrowserSecurityServices {
        sessions: SessionSecurityServices {
            store: match config.http.session.store {
                ConfigSessionStore::Memory => SessionStoreTopology::Memory,
                ConfigSessionStore::Database => SessionStoreTopology::Database,
                ConfigSessionStore::Redis => SessionStoreTopology::Redis,
                ConfigSessionStore::Valkey => SessionStoreTopology::Valkey,
            },
            idle_timeout: Duration::from_secs(config.http.session.idle_timeout_secs),
            absolute_timeout: Duration::from_secs(config.http.session.absolute_timeout_secs),
            session_cookie: CookiePolicy::from_config(
                &config.http.session_cookie,
                CookieProtection::Signed,
            ),
            flash_cookie: CookiePolicy::from_config(
                &config.http.flash_cookie,
                CookieProtection::Signed,
            ),
        },
        csrf: CsrfProtection::from_config(&config.http.csrf),
    }
}

fn observability_runtime_from_config(config: &PlatformConfig) -> ObservabilityRuntimeServices {
    let mut runtime = ObservabilityRuntime::baseline(&config.observability, config.app.environment)
        .expect("baseline observability runtime must be valid");

    runtime.liveness = HealthReport::new(HealthProbeKind::Liveness);

    let mut readiness = HealthReport::new(HealthProbeKind::Readiness)
        .with_dependency(DependencyKind::Database, true, DependencyStatus::Healthy)
        .expect("database dependency must be unique")
        .with_dependency(
            DependencyKind::ExtensionRegistry,
            true,
            DependencyStatus::Healthy,
        )
        .expect("extension registry dependency must be unique")
        .with_dependency(DependencyKind::Queue, true, DependencyStatus::Healthy)
        .expect("queue dependency must be unique");

    if config.cache.l2.is_some()
        || matches!(
            config.http.session.store,
            ConfigSessionStore::Redis | ConfigSessionStore::Valkey
        )
    {
        readiness = readiness
            .with_dependency(
                DependencyKind::DistributedCache,
                true,
                DependencyStatus::Healthy,
            )
            .expect("distributed cache dependency must be unique");
    }

    if config.storage.object_store.is_some() {
        readiness = readiness
            .with_dependency(DependencyKind::ObjectStore, true, DependencyStatus::Healthy)
            .expect("object store dependency must be unique");
    }

    if config.storage.object_store_secret.is_some()
        || config.auth.tuple_store_secret.is_some()
        || config.tls.provider.is_some()
    {
        readiness = readiness
            .with_dependency(DependencyKind::Secrets, true, DependencyStatus::Healthy)
            .expect("secrets dependency must be unique");
    }

    if config.tls.mode != TlsMode::External {
        readiness = readiness
            .with_dependency(DependencyKind::Tls, true, DependencyStatus::Healthy)
            .expect("tls dependency must be unique");
    }

    runtime.readiness = readiness;
    runtime.maintenance = MaintenanceMode::disabled();
    runtime
}

fn jobs_runtime_from_config(config: &PlatformConfig) -> JobsRuntimeServices {
    JobsRuntime::from_config(&config.jobs).expect("jobs runtime config must be valid")
}

fn data_runtime_from_config(config: &PlatformConfig) -> DataRuntimeServices {
    DataRuntime::from_config(&config.database).expect("data runtime config must be valid")
}

fn cli_runtime_from_config(config: &PlatformConfig) -> CliRuntimeServices {
    CliRuntime::baseline(&config.app.name).expect("cli runtime config must be valid")
}

fn tls_runtime_from_config(config: &PlatformConfig) -> TlsRuntimeServices {
    TlsRuntime::from_config(&config.tls)
}

fn template_runtime_services() -> TemplateRuntimeServices {
    let registry = TemplateRegistry::new();

    TemplateRuntimeServices {
        customer_app_namespace: TemplateNamespace::new("customer-app")
            .expect("constant template namespace is valid"),
        core_namespace: TemplateNamespace::new("core")
            .expect("constant template namespace is valid"),
        runtime: TemplateRuntime::new(registry.clone()),
        registry,
    }
}

fn i18n_runtime_from_config(config: &PlatformConfig) -> I18nRuntimeServices {
    let default_locale =
        LocaleTag::new(config.i18n.default_locale.clone()).expect("validated locale");
    let supported_locales = config
        .i18n
        .supported_locales
        .iter()
        .cloned()
        .map(LocaleTag::new)
        .collect::<Result<Vec<_>, _>>()
        .expect("validated locales");
    let fallback_locale =
        LocaleTag::new(config.i18n.fallback_locale.clone()).expect("validated locale");
    let router = LocaleRouter::new(
        LocaleUrlConfig::path_prefix(config.seo.canonical_host.clone())
            .expect("validated canonical host"),
    );
    let translations = TranslationRuntime::new(
        default_locale.clone(),
        supported_locales
            .iter()
            .cloned()
            .map(|locale| {
                TranslationCatalog::new(
                    locale.clone(),
                    vec![(
                        davenda_i18n::MessageKey::new("core.locale").expect("static key"),
                        locale.to_string(),
                    )],
                )
                .expect("static catalog")
            })
            .collect::<Vec<_>>(),
    )
    .expect("default translation runtime");

    I18nRuntimeServices {
        default_locale,
        supported_locales,
        fallback_locale,
        router,
        translations,
    }
}

fn seo_runtime_from_config(config: &PlatformConfig) -> SeoRuntimeServices {
    SeoRuntimeServices {
        canonical_host: config.seo.canonical_host.clone(),
        emit_json_ld: config.seo.emit_json_ld,
        sitemap_enabled: config.seo.sitemap_enabled,
    }
}

fn a11y_runtime_services() -> A11yRuntimeServices {
    A11yRuntimeServices {
        navigation: NavigationContract::standard(),
        theme_baseline: ThemeAccessibilityContract::new(4.5, 3.0, 3.0, true, true)
            .expect("static baseline"),
    }
}

fn wasm_runtime_from_config(config: &PlatformConfig) -> WasmRuntimeServices {
    let request_limit = Duration::from_millis(config.wasm.default_time_limit_ms);
    let tighten = |point| tighten_runtime_limit(ResourceLimits::baseline_for(point), request_limit);

    WasmRuntimeServices {
        extension_directory: config.wasm.directory.clone(),
        allow_network: config.wasm.allow_network,
        limits: WasmLimitsProfile {
            page: tighten(ExtensionPointKind::Page),
            api: tighten(ExtensionPointKind::Api),
            admin_widget: tighten(ExtensionPointKind::AdminWidget),
            render_hook: tighten(ExtensionPointKind::RenderHook),
            webhook: tighten(ExtensionPointKind::Webhook),
            job: ResourceLimits::baseline_for(ExtensionPointKind::Job),
            scheduled_job: ResourceLimits::baseline_for(ExtensionPointKind::ScheduledJob),
        },
    }
}

fn tighten_runtime_limit(mut limits: ResourceLimits, max_runtime: Duration) -> ResourceLimits {
    if max_runtime < limits.max_runtime {
        limits.max_runtime = max_runtime;
    }

    limits
}

fn currency_for_locale(locale: &LocaleTag) -> CurrencyCode {
    let currency = match locale.as_str() {
        "fr-FR" => "EUR",
        _ => "GBP",
    };
    CurrencyCode::new(currency).expect("static currency")
}

fn timezone_for_locale(locale: &LocaleTag) -> TimeZoneId {
    let timezone = match locale.as_str() {
        "fr-FR" => "Europe/Paris",
        _ => "Europe/London",
    };
    TimeZoneId::new(timezone).expect("static timezone")
}

fn sign_payload(secret: &[u8], payload: &[u8]) -> Result<String, BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn verify_payload(
    secret: &[u8],
    payload: &[u8],
    signature: &str,
) -> Result<(), BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| BrowserSecurityError::InvalidCookieSignature)?;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    mac.verify_slice(&signature)
        .map_err(|_| BrowserSecurityError::InvalidCookieSignature)
}
