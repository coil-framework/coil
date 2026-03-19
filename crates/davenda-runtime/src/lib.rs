use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::time::Duration;

use davenda_auth::AuthModelPackage;
use davenda_cache::{
    ApplicationCachePolicy, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError,
    CacheNamespace, CachePlan, CachePlanRequest, CachePlanner, CacheRuntime, CacheScope,
    CacheTopology, EntityTag, FillDecision, FreshnessPolicy, HttpCachePolicy, InvalidationSet,
    InvalidationTag, ResponseValidators,
};
use davenda_config::{ConfigError, PlatformConfig};
use davenda_core::{
    BrowserSecurityServices, BulkOperationDefinition, CapabilityValidationError,
    CliRuntimeServices, CookieSigner, DataRuntimeServices, EventSubscription, HttpFileDeliveryMode,
    HttpResponseContract, HttpSurfaceArea, HttpSurfaceContribution, HttpSurfaceMethod, JobContract,
    JobTriggerKind, JobsRuntimeServices, ModuleInstallationError, ModuleManifest,
    ObservabilityRuntimeServices, PlatformModule, RegistrationError, ReportDefinition,
    SearchIndexContribution, ServiceDescriptor, TemplateRuntimeServices, TlsRuntimeServices,
    WasmRuntimeServices, bootstrap_core_services, validate_module_capabilities,
    validate_module_installation,
};
use davenda_data::{DataModelError, MigrationPlan};
use davenda_jobs::{
    DeadLetterReason, DomainEventEnvelope, DomainEventId, DomainEventType, EventHandlerId,
    EventHandlerMetadata, EventSubscriptionId, EventSubscriptionMetadata, IdempotencyKey,
    JobFailureDisposition, JobId, JobInstant, JobLease, JobName, JobQueueName, JobSpec,
    JobsCoordinator, JobsDomain, JobsModelError, QueueTopology, RetryPolicy, SchedulerLeadership,
};
use davenda_observability::{
    CustomerAppId, FeatureFlag, FeatureFlagContext, FeatureFlagId, MaintenanceMode,
    ObservabilityError,
};
use davenda_ops::{
    BulkOperationPlan, BulkOperationRequest, OpsCatalog, OpsModelError, OpsPlanner,
    ReportExportPlan, ReportExportRequest,
};
use davenda_tls::{
    CertificateId, CertificateInventory, CertificateProviderKind, CertificateRecord,
    ChallengeTicket, EdgeMode, HostnameBinding, HotReloadEvent, IssuancePlan, RenewalPlan,
    TlsAutomationRuntime, TlsInstant, TlsModelError,
};
use davenda_wasm::{
    ContractVersion, ExtensionPointKind, ExtensionRegistry, InstalledExtension, WasmModelError,
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub const fn is_state_changing(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteArea {
    Public,
    Account,
    Admin,
    Api,
    Fragment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    Any,
    Exact(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalePolicy {
    DefaultOnly,
    Localized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAuthGate {
    Public,
    Session,
    Capability(davenda_auth::Capability),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDefinition {
    pub name: String,
    pub method: HttpMethod,
    pub path: String,
    pub area: RouteArea,
    pub host: HostPattern,
    pub locale_policy: LocalePolicy,
    pub auth: RouteAuthGate,
    pub module: Option<String>,
    pub feature_flag: Option<String>,
}

impl RouteDefinition {
    pub fn new(
        name: impl Into<String>,
        method: HttpMethod,
        path: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        let name = validate_route_name(name.into())?;
        let path = validate_route_path(path.into())?;

        Ok(Self {
            name,
            method,
            path,
            area: RouteArea::Public,
            host: HostPattern::Any,
            locale_policy: LocalePolicy::DefaultOnly,
            auth: RouteAuthGate::Public,
            module: None,
            feature_flag: None,
        })
    }

    pub fn with_area(mut self, area: RouteArea) -> Self {
        self.area = area;
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Result<Self, RouteBuildError> {
        self.host = HostPattern::Exact(validate_host(host.into())?);
        Ok(self)
    }

    pub fn localized(mut self) -> Self {
        self.locale_policy = LocalePolicy::Localized;
        self
    }

    pub fn requiring_session(mut self) -> Self {
        self.auth = RouteAuthGate::Session;
        self
    }

    pub fn requiring_capability(mut self, capability: davenda_auth::Capability) -> Self {
        self.auth = RouteAuthGate::Capability(capability);
        self
    }

    pub fn from_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    pub fn with_feature_flag(mut self, feature_flag: impl Into<String>) -> Self {
        self.feature_flag = Some(feature_flag.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiddlewareStage {
    TransportNormalization,
    CustomerAppResolution,
    TraceContext,
    LocaleResolution,
    SessionResolution,
    BrowserPolicy,
    ResponsePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRuntimePlan {
    pub middleware: Vec<MiddlewareStage>,
    pub routes: Vec<RouteDefinition>,
}

impl HttpRuntimePlan {
    pub fn resolve(
        &self,
        config: &PlatformConfig,
        method: HttpMethod,
        host: &str,
        path: &str,
    ) -> Option<ResolvedRoute> {
        self.resolve_match(config, method, host, path)
            .map(|matched| matched.resolved)
    }

    pub fn resolve_match(
        &self,
        config: &PlatformConfig,
        method: HttpMethod,
        host: &str,
        path: &str,
    ) -> Option<ResolvedRouteMatch> {
        self.routes.iter().find_map(|route| {
            if route.method != method {
                return None;
            }

            if let HostPattern::Exact(expected) = &route.host {
                if expected != host {
                    return None;
                }
            }

            match route.locale_policy {
                LocalePolicy::DefaultOnly => match match_route_path(&route.path, path) {
                    Some(params) => Some(ResolvedRouteMatch {
                        route: route.clone(),
                        resolved: ResolvedRoute {
                            route_name: route.name.clone(),
                            locale: None,
                            auth: route.auth,
                            params,
                        },
                    }),
                    None => None,
                },
                LocalePolicy::Localized if config.i18n.localized_routes => {
                    config.i18n.supported_locales.iter().find_map(|locale| {
                        let localized_path = format!(
                            "/{}/{}",
                            locale.trim_matches('/'),
                            route.path.trim_start_matches('/')
                        );
                        match_route_path(&localized_path, path).map(|params| ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                locale: Some(locale.clone()),
                                auth: route.auth,
                                params,
                            },
                        })
                    })
                }
                LocalePolicy::Localized => None,
            }
        })
    }

    pub fn path_for(
        &self,
        config: &PlatformConfig,
        route_name: &str,
        params: &BTreeMap<String, String>,
        locale: Option<&str>,
    ) -> Result<String, RouteUrlError> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| RouteUrlError::UnknownRoute {
                route: route_name.to_string(),
            })?;
        let rendered_path = render_route_path(&route.path, params, route_name)?;

        if route.locale_policy == LocalePolicy::Localized {
            let locale = locale.unwrap_or(&config.i18n.default_locale);
            if !config
                .i18n
                .supported_locales
                .iter()
                .any(|item| item == locale)
            {
                return Err(RouteUrlError::UnsupportedLocale {
                    route: route_name.to_string(),
                    locale: locale.to_string(),
                });
            }

            return Ok(format!(
                "/{}/{}",
                locale.trim_matches('/'),
                rendered_path.trim_start_matches('/')
            ));
        }

        Ok(rendered_path)
    }

    pub fn absolute_url_for(
        &self,
        config: &PlatformConfig,
        route_name: &str,
        params: &BTreeMap<String, String>,
        locale: Option<&str>,
    ) -> Result<String, RouteUrlError> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| RouteUrlError::UnknownRoute {
                route: route_name.to_string(),
            })?;
        let path = self.path_for(config, route_name, params, locale)?;
        let host = match &route.host {
            HostPattern::Exact(host) => host.as_str(),
            HostPattern::Any => config.seo.canonical_host.as_str(),
        };
        Ok(format!("https://{host}{path}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub route_name: String,
    pub locale: Option<String>,
    pub auth: RouteAuthGate,
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRouteMatch {
    pub route: RouteDefinition,
    pub resolved: ResolvedRoute,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInput {
    pub method: HttpMethod,
    pub host: String,
    pub path: String,
    pub scheme: String,
    pub forwarded_proto: Option<String>,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub session_cookie: Option<String>,
    pub csrf_token: Option<String>,
    pub csrf_action: Option<String>,
    pub maintenance_bypass_token: Option<String>,
    pub principal_id: Option<String>,
    pub granted_capabilities: HashSet<davenda_auth::Capability>,
}

impl RequestInput {
    pub fn new(
        method: HttpMethod,
        host: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            method,
            host: validate_host(host.into())?,
            path: validate_route_path(path.into())?,
            scheme: "https".to_string(),
            forwarded_proto: None,
            request_id: None,
            session_id: None,
            session_cookie: None,
            csrf_token: None,
            csrf_action: None,
            maintenance_bypass_token: None,
            principal_id: None,
            granted_capabilities: HashSet::new(),
        })
    }

    pub fn with_scheme(mut self, scheme: impl Into<String>) -> Self {
        self.scheme = scheme.into();
        self
    }

    pub fn with_forwarded_proto(mut self, proto: impl Into<String>) -> Self {
        self.forwarded_proto = Some(proto.into());
        self
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    pub fn with_session_cookie(mut self, session_cookie: impl Into<String>) -> Self {
        self.session_cookie = Some(session_cookie.into());
        self
    }

    pub fn with_csrf_token(mut self, csrf_token: impl Into<String>) -> Self {
        self.csrf_token = Some(csrf_token.into());
        self
    }

    pub fn with_csrf_action(mut self, csrf_action: impl Into<String>) -> Self {
        self.csrf_action = Some(csrf_action.into());
        self
    }

    pub fn with_maintenance_bypass_token(mut self, bypass_token: impl Into<String>) -> Self {
        self.maintenance_bypass_token = Some(bypass_token.into());
        self
    }

    pub fn with_principal(mut self, principal_id: impl Into<String>) -> Self {
        self.principal_id = Some(principal_id.into());
        self
    }

    pub fn grant_capability(mut self, capability: davenda_auth::Capability) -> Self {
        self.granted_capabilities.insert(capability);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestTraceContext {
    pub request_id: String,
    pub transport_scheme: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionContext {
    pub session_id: Option<String>,
    pub resolved_from_cookie: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalContext {
    pub principal_id: Option<String>,
    pub granted_capabilities: HashSet<davenda_auth::Capability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheDisposition {
    Public,
    Private,
    Uncacheable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestExecution {
    pub customer_app: String,
    pub route: ResolvedRoute,
    pub route_area: RouteArea,
    pub locale: String,
    pub trace: RequestTraceContext,
    pub session: SessionContext,
    pub principal: PrincipalContext,
    pub cache: CacheDisposition,
    pub cache_plan: ExecutedCachePlan,
    pub middleware: Vec<MiddlewareStage>,
    pub response: HandlerResponse,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutedCachePlan {
    pub plan: CachePlan,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageResponse {
    pub template: String,
    pub status: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentResponse {
    pub template: String,
    pub fragment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectResponse {
    pub location: String,
    pub status: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonResponse {
    pub status: u16,
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResponse {
    pub logical_path: String,
    pub content_type: String,
    pub delivery_mode: FileDeliveryMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerResponse {
    Page(PageResponse),
    Fragment(FragmentResponse),
    Redirect(RedirectResponse),
    Json(JsonResponse),
    File(FileResponse),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerDefinition {
    pub route_name: String,
    pub response: HandlerResponse,
}

impl HandlerDefinition {
    pub fn page(
        route_name: impl Into<String>,
        template: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Page(PageResponse {
                template: validate_template_name(template.into())?,
                status: 200,
            }),
        })
    }

    pub fn fragment(
        route_name: impl Into<String>,
        template: impl Into<String>,
        fragment_id: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Fragment(FragmentResponse {
                template: validate_template_name(template.into())?,
                fragment_id: validate_fragment_id(fragment_id.into())?,
            }),
        })
    }

    pub fn redirect(
        route_name: impl Into<String>,
        location: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Redirect(RedirectResponse {
                location: validate_route_path(location.into())?,
                status: 303,
            }),
        })
    }

    pub fn json(
        route_name: impl Into<String>,
        payload: BTreeMap<String, String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Json(JsonResponse {
                status: 200,
                payload,
            }),
        })
    }

    pub fn file(
        route_name: impl Into<String>,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: FileDeliveryMode,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::File(FileResponse {
                logical_path: validate_template_name(logical_path.into())?,
                content_type: validate_template_name(content_type.into())?,
                delivery_mode,
            }),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteBuildError {
    #[error("route names must not be empty")]
    EmptyRouteName,
    #[error("route paths must start with `/`, got `{path}`")]
    InvalidRoutePath { path: String },
    #[error("host pattern must not be empty")]
    EmptyHostPattern,
    #[error("route `{name}` is registered more than once for method {method:?}")]
    DuplicateRoute { name: String, method: HttpMethod },
    #[error(
        "route `{name}` requires capability `{capability}` but the auth package does not bind it"
    )]
    MissingCapabilityBinding {
        name: String,
        capability: davenda_auth::Capability,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RequestExecutionError {
    #[error("no route matches {method:?} {host}{path}")]
    RouteNotFound {
        method: HttpMethod,
        host: String,
        path: String,
    },
    #[error("route `{route}` requires a resolved session")]
    SessionRequired { route: String },
    #[error("route `{route}` requires capability `{capability}`")]
    CapabilityRequired {
        route: String,
        capability: davenda_auth::Capability,
    },
    #[error("route `{route}` requires a CSRF token")]
    MissingCsrfToken { route: String },
    #[error("route `{route}` requires a session before CSRF can be validated")]
    MissingSessionForCsrf { route: String },
    #[error("route `{route}` supplied an invalid CSRF token")]
    InvalidCsrfToken { route: String },
    #[error("session cookie failed validation: {0}")]
    InvalidSessionCookie(String),
    #[error("route `{route}` is disabled by maintenance mode")]
    MaintenanceMode { route: String },
    #[error("route `{route}` is disabled because feature flag `{feature_flag}` is not enabled")]
    FeatureFlagDisabled { route: String, feature_flag: String },
    #[error("route `{route}` has no registered handler")]
    HandlerNotRegistered { route: String },
    #[error(transparent)]
    Cache(#[from] CacheModelError),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteUrlError {
    #[error("route `{route}` is not registered")]
    UnknownRoute { route: String },
    #[error("route `{route}` requires parameter `{parameter}`")]
    MissingRouteParameter { route: String, parameter: String },
    #[error("route `{route}` does not support locale `{locale}`")]
    UnsupportedLocale { route: String, locale: String },
}

pub struct RuntimeBuilder<P> {
    config: PlatformConfig,
    auth_package: P,
    modules: Vec<Box<dyn PlatformModule>>,
    extensions: Vec<InstalledExtension>,
    routes: Vec<RouteDefinition>,
    handlers: Vec<HandlerDefinition>,
    feature_flags: Vec<FeatureFlag>,
    maintenance_mode: Option<MaintenanceMode>,
}

impl<P> RuntimeBuilder<P>
where
    P: AuthModelPackage,
{
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            config,
            auth_package,
            modules: Vec::new(),
            extensions: Vec::new(),
            routes: Vec::new(),
            handlers: Vec::new(),
            feature_flags: Vec::new(),
            maintenance_mode: None,
        }
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn with_boxed_module(mut self, module: Box<dyn PlatformModule>) -> Self {
        self.modules.push(module);
        self
    }

    pub fn with_installed_extension(mut self, extension: InstalledExtension) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn with_route(mut self, route: RouteDefinition) -> Self {
        self.routes.push(route);
        self
    }

    pub fn with_handler(mut self, handler: HandlerDefinition) -> Self {
        self.handlers.push(handler);
        self
    }

    pub fn with_feature_flag(mut self, feature_flag: FeatureFlag) -> Self {
        self.feature_flags.push(feature_flag);
        self
    }

    pub fn with_maintenance_mode(mut self, maintenance_mode: MaintenanceMode) -> Self {
        self.maintenance_mode = Some(maintenance_mode);
        self
    }

    pub fn build(self) -> Result<RuntimePlan, RuntimeBuildError> {
        self.config.validate().map_err(ConfigError::Validation)?;

        if self.auth_package.manifest().name != self.config.auth.package {
            return Err(RuntimeBuildError::AuthPackageMismatch {
                configured: self.config.auth.package,
                actual: self.auth_package.manifest().name.clone(),
            });
        }

        let bootstrap = bootstrap_core_services(&self.config)?;
        let mut registry = bootstrap.registry;
        let mut observability = bootstrap.observability;
        let mut module_manifests = Vec::new();
        let mut install_migrations = MigrationPlan::new();

        for feature_flag in self.feature_flags {
            observability.flags.insert(feature_flag)?;
        }

        if let Some(maintenance_mode) = self.maintenance_mode {
            observability.maintenance = maintenance_mode;
        }

        let mut installed_modules = Vec::new();
        let mut collected_modules = Vec::new();

        for module in self.modules {
            let manifest = module.manifest();
            validate_module_capabilities(&self.auth_package, &manifest)?;
            installed_modules.push(manifest.name.clone());
            collected_modules.push((module, manifest));
        }

        let core_service_id_storage = registry
            .services()
            .map(|service| service.id.clone())
            .collect::<Vec<_>>();
        let core_service_ids = core_service_id_storage
            .iter()
            .map(|service_id| service_id.as_str())
            .collect::<Vec<_>>();

        for (_, manifest) in &collected_modules {
            validate_module_installation(manifest, &installed_modules, &core_service_ids)?;
            registry.register_module_manifest(manifest.clone())?;
            module_manifests.push(manifest.clone());
        }

        let (module_routes, module_handlers) = module_http_contributions(&module_manifests)?;
        let mut all_routes = self.routes;
        all_routes.extend(module_routes);
        let mut all_handlers = self.handlers;
        all_handlers.extend(module_handlers);
        let http = build_http_runtime_plan(&self.auth_package, &all_routes)?;
        let handlers = build_handler_registry(&all_routes, all_handlers)?;

        for (module, _) in collected_modules {
            if let Some(plan) = module.install_migration_plan() {
                for step in plan.ordered_steps().iter().cloned() {
                    install_migrations.insert(step)?;
                }
            }
            module.register(&mut registry)?;
        }

        let module_jobs = module_manifests
            .iter()
            .flat_map(|manifest| {
                manifest
                    .jobs
                    .iter()
                    .cloned()
                    .map(|job| RegisteredModuleJob {
                        module: manifest.name.clone(),
                        job,
                    })
            })
            .collect::<Vec<_>>();
        let module_event_subscriptions = module_manifests
            .iter()
            .flat_map(|manifest| {
                manifest
                    .event_subscriptions
                    .iter()
                    .cloned()
                    .map(|subscription| RegisteredEventSubscription {
                        module: manifest.name.clone(),
                        subscription,
                    })
            })
            .collect::<Vec<_>>();
        let module_search_contributions = module_manifests
            .iter()
            .flat_map(|manifest| {
                manifest
                    .search_contributions
                    .iter()
                    .cloned()
                    .map(|contribution| RegisteredSearchContribution {
                        module: manifest.name.clone(),
                        contribution,
                    })
            })
            .collect::<Vec<_>>();
        let module_report_definitions = module_manifests
            .iter()
            .flat_map(|manifest| {
                manifest
                    .report_definitions
                    .iter()
                    .cloned()
                    .map(|definition| RegisteredReportDefinition {
                        module: manifest.name.clone(),
                        definition,
                    })
            })
            .collect::<Vec<_>>();
        let module_bulk_operations =
            module_manifests
                .iter()
                .flat_map(|manifest| {
                    manifest.bulk_operations.iter().cloned().map(|definition| {
                        RegisteredBulkOperation {
                            module: manifest.name.clone(),
                            definition,
                        }
                    })
                })
                .collect::<Vec<_>>();
        let ops_catalog = OpsCatalog::from_manifests(&module_manifests)?;
        let (registered_runtime_jobs, registered_runtime_event_subscriptions, jobs_domain) =
            build_runtime_jobs_domain(&bootstrap.jobs, &module_jobs, &module_event_subscriptions)?;
        let registered_extension_slots = collect_extension_slots(&module_manifests)?;
        let mut extension_registry = ExtensionRegistry::new(ContractVersion::new(1, 0, 0));
        let mut installed_extensions = Vec::new();

        for extension in self.extensions {
            if extension.customer_app_id() != self.config.app.name {
                return Err(RuntimeBuildError::ExtensionCustomerAppMismatch {
                    extension_id: extension.manifest().id.to_string(),
                    configured: self.config.app.name.clone(),
                    actual: extension.customer_app_id().to_string(),
                });
            }

            installed_extensions.push(InstalledExtensionSummary {
                extension_id: extension.manifest().id.to_string(),
                display_name: extension.manifest().display_name.clone(),
                customer_app_id: extension.customer_app_id().to_string(),
                handler_count: extension.installed_handler_count(),
            });
            extension_registry.install(extension)?;
        }

        for handler in extension_registry.registered_handlers() {
            validate_extension_handler_slot(handler, &registered_extension_slots)?;
        }

        Ok(RuntimePlan {
            config: self.config,
            auth_package_name: self.auth_package.manifest().name.clone(),
            cache_topology: bootstrap.cache.topology,
            cache_planner: bootstrap.cache.planner,
            browser: bootstrap.browser,
            cli: bootstrap.cli,
            data: bootstrap.data,
            jobs: bootstrap.jobs,
            observability,
            http,
            handlers,
            template: bootstrap.template,
            tls: bootstrap.tls,
            wasm: bootstrap.wasm,
            services: registry.services().cloned().collect(),
            modules: module_manifests,
            install_migrations,
            extension_registry,
            registered_extension_slots,
            installed_extensions,
            module_jobs,
            module_event_subscriptions,
            module_search_contributions,
            module_report_definitions,
            module_bulk_operations,
            registered_runtime_jobs,
            registered_runtime_event_subscriptions,
            jobs_domain,
            ops_catalog,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub cache_topology: CacheTopology,
    pub cache_planner: CachePlanner,
    pub browser: BrowserSecurityServices,
    pub cli: CliRuntimeServices,
    pub data: DataRuntimeServices,
    pub jobs: JobsRuntimeServices,
    pub observability: ObservabilityRuntimeServices,
    pub http: HttpRuntimePlan,
    pub handlers: BTreeMap<String, HandlerDefinition>,
    pub template: TemplateRuntimeServices,
    pub tls: TlsRuntimeServices,
    pub wasm: WasmRuntimeServices,
    pub services: Vec<ServiceDescriptor>,
    pub modules: Vec<ModuleManifest>,
    pub install_migrations: MigrationPlan,
    pub extension_registry: ExtensionRegistry,
    pub registered_extension_slots: Vec<RegisteredExtensionSlot>,
    pub installed_extensions: Vec<InstalledExtensionSummary>,
    pub module_jobs: Vec<RegisteredModuleJob>,
    pub module_event_subscriptions: Vec<RegisteredEventSubscription>,
    pub module_search_contributions: Vec<RegisteredSearchContribution>,
    pub module_report_definitions: Vec<RegisteredReportDefinition>,
    pub module_bulk_operations: Vec<RegisteredBulkOperation>,
    pub registered_runtime_jobs: Vec<RuntimeJobDefinition>,
    pub registered_runtime_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
    pub jobs_domain: JobsDomain,
    pub ops_catalog: OpsCatalog,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredExtensionSlot {
    pub module: String,
    pub kind: ExtensionPointKind,
    pub surface: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtensionSummary {
    pub extension_id: String,
    pub display_name: String,
    pub customer_app_id: String,
    pub handler_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredModuleJob {
    pub module: String,
    pub job: JobContract,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredEventSubscription {
    pub module: String,
    pub subscription: EventSubscription,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSearchContribution {
    pub module: String,
    pub contribution: SearchIndexContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredReportDefinition {
    pub module: String,
    pub definition: ReportDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredBulkOperation {
    pub module: String,
    pub definition: BulkOperationDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeJobDefinition {
    pub module: String,
    pub contract: JobContract,
    pub queue: JobQueueName,
    pub retry_policy: RetryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEventSubscriptionDefinition {
    pub module: String,
    pub event_type: DomainEventType,
    pub subscription_id: EventSubscriptionId,
    pub handler_id: EventHandlerId,
    pub job_name: String,
    pub reaction_queue: JobQueueName,
    pub retry_policy: RetryPolicy,
    pub target_trigger: JobTriggerKind,
    pub target_queue: JobQueueName,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobDispatchRequest {
    pub job_name: String,
    pub payload_description: String,
    pub scheduled_for: Option<JobInstant>,
    pub idempotency_key: Option<String>,
}

impl JobDispatchRequest {
    pub fn new(
        job_name: impl Into<String>,
        payload_description: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        let job_name = validate_runtime_identifier("job_name", job_name.into())?;
        let payload_description =
            validate_runtime_identifier("payload_description", payload_description.into())?;

        Ok(Self {
            job_name,
            payload_description,
            scheduled_for: None,
            idempotency_key: None,
        })
    }

    pub fn scheduled_for(mut self, instant: JobInstant) -> Self {
        self.scheduled_for = Some(instant);
        self
    }

    pub fn with_idempotency_key(
        mut self,
        key: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        self.idempotency_key = Some(validate_runtime_identifier("idempotency_key", key.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventDispatchRequest {
    pub event_type: String,
    pub aggregate_kind: String,
    pub aggregate_id: String,
    pub payload_description: String,
    pub correlation_id: Option<String>,
    pub causation_id: Option<String>,
}

impl DomainEventDispatchRequest {
    pub fn new(
        event_type: impl Into<String>,
        aggregate_kind: impl Into<String>,
        aggregate_id: impl Into<String>,
        payload_description: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        Ok(Self {
            event_type: validate_runtime_identifier("event_type", event_type.into())?,
            aggregate_kind: validate_runtime_identifier("aggregate_kind", aggregate_kind.into())?,
            aggregate_id: validate_runtime_identifier("aggregate_id", aggregate_id.into())?,
            payload_description: validate_runtime_identifier(
                "payload_description",
                payload_description.into(),
            )?,
            correlation_id: None,
            causation_id: None,
        })
    }

    pub fn with_correlation_id(
        mut self,
        correlation_id: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        self.correlation_id = Some(validate_runtime_identifier(
            "correlation_id",
            correlation_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_causation_id(
        mut self,
        causation_id: impl Into<String>,
    ) -> Result<Self, RuntimeJobsError> {
        self.causation_id = Some(validate_runtime_identifier(
            "causation_id",
            causation_id.into(),
        )?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventDispatch {
    pub event_id: DomainEventId,
    pub event_type: DomainEventType,
    pub enqueued_jobs: Vec<JobId>,
}

#[derive(Debug, Clone)]
pub struct JobsHost {
    pub customer_app: String,
    pub scheduler_node_id: String,
    pub runtime: JobsRuntimeServices,
    pub queue_topology: QueueTopology,
    pub registered_jobs: Vec<RuntimeJobDefinition>,
    pub registered_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
    pub jobs_domain: JobsDomain,
    coordinator: JobsCoordinator,
    next_job_sequence: u64,
    next_event_sequence: u64,
}

impl JobsHost {
    pub fn enqueue_spec(
        &mut self,
        spec: JobSpec,
        now: JobInstant,
    ) -> Result<JobId, RuntimeJobsError> {
        let job_id = spec.job_id.clone();
        self.coordinator.enqueue(spec, now)?;
        Ok(job_id)
    }

    pub fn enqueue_job(
        &mut self,
        request: JobDispatchRequest,
        now: JobInstant,
    ) -> Result<JobId, RuntimeJobsError> {
        let Some(definition) = self
            .registered_jobs
            .iter()
            .find(|definition| definition.contract.name == request.job_name)
            .cloned()
        else {
            return Err(RuntimeJobsError::UnknownJob {
                job: request.job_name,
            });
        };

        match definition.contract.trigger {
            JobTriggerKind::Scheduled if request.scheduled_for.is_none() => {
                return Err(RuntimeJobsError::ScheduledJobRequiresSchedule {
                    job: definition.contract.name,
                });
            }
            JobTriggerKind::Scheduled => {}
            JobTriggerKind::DomainEvent => {
                return Err(RuntimeJobsError::DomainEventJobRequiresEventDispatch {
                    job: definition.contract.name,
                });
            }
            trigger if request.scheduled_for.is_some() => {
                return Err(RuntimeJobsError::UnexpectedSchedule {
                    job: definition.contract.name,
                    trigger,
                });
            }
            _ => {}
        }

        let mut spec = JobSpec::new(
            self.issue_job_id(&definition.contract.name)?,
            JobName::new(definition.contract.name.clone())?,
            definition.queue.clone(),
            request.payload_description,
        )?
        .with_retry_policy(definition.retry_policy.clone());

        if let Some(scheduled_for) = request.scheduled_for {
            spec = spec.scheduled_for(scheduled_for);
        }

        match request.idempotency_key {
            Some(key) => {
                spec = spec.with_idempotency_key(IdempotencyKey::new(key)?);
            }
            None if definition.retry_policy.is_retrying() => {
                return Err(RuntimeJobsError::MissingIdempotencyKey {
                    job: definition.contract.name,
                });
            }
            None => {}
        }

        let job_id = spec.job_id.clone();
        self.coordinator.enqueue(spec, now)?;
        Ok(job_id)
    }

    pub fn emit_domain_event(
        &mut self,
        request: DomainEventDispatchRequest,
        now: JobInstant,
    ) -> Result<DomainEventDispatch, RuntimeJobsError> {
        let event_type = DomainEventType::new(request.event_type.clone())?;
        let event_id = self.issue_event_id(&request.event_type)?;
        let mut envelope = DomainEventEnvelope::new(
            event_id.clone(),
            event_type.clone(),
            request.aggregate_kind,
            request.aggregate_id,
            now,
            request.payload_description,
        )?;

        if let Some(correlation_id) = request.correlation_id {
            envelope = envelope.with_correlation_id(correlation_id)?;
        }

        if let Some(causation_id) = request.causation_id {
            envelope = envelope.with_causation_id(causation_id)?;
        }

        let mut enqueued_jobs = Vec::new();
        for subscription in self
            .registered_event_subscriptions
            .iter()
            .filter(|subscription| subscription.event_type == event_type)
            .cloned()
        {
            let mut spec = JobSpec::new(
                JobId::new(format!(
                    "event:{}:{}",
                    event_id.as_str(),
                    subscription.subscription_id.as_str()
                ))?,
                JobName::new(format!("event-handler:{}", subscription.job_name))?,
                subscription.reaction_queue,
                format!(
                    "dispatch {} for {}:{}",
                    event_type.as_str(),
                    envelope.aggregate_kind,
                    envelope.aggregate_id
                ),
            )?
            .with_retry_policy(subscription.retry_policy.clone());

            if subscription.retry_policy.is_retrying() {
                spec = spec.with_idempotency_key(IdempotencyKey::new(format!(
                    "event:{}:{}:{}",
                    event_id.as_str(),
                    subscription.module,
                    subscription.job_name
                ))?);
            }

            let job_id = spec.job_id.clone();
            self.coordinator.enqueue(spec, now)?;
            enqueued_jobs.push(job_id);
        }

        Ok(DomainEventDispatch {
            event_id,
            event_type,
            enqueued_jobs,
        })
    }

    pub fn acquire_scheduler_leadership(
        &mut self,
        now: JobInstant,
        lease_ttl: std::time::Duration,
    ) -> Result<SchedulerLeadership, RuntimeJobsError> {
        Ok(self.coordinator.acquire_scheduler_leadership(
            self.scheduler_node_id.clone(),
            now,
            lease_ttl,
        )?)
    }

    pub fn promote_due_jobs(&mut self, now: JobInstant) -> Result<Vec<JobId>, RuntimeJobsError> {
        Ok(self
            .coordinator
            .promote_due_jobs(&self.scheduler_node_id, now)?)
    }

    pub fn lease_ready_jobs(
        &mut self,
        queue: &JobQueueName,
        worker_id: impl Into<String>,
        now: JobInstant,
        lease_ttl: std::time::Duration,
        max_jobs: usize,
    ) -> Result<Vec<JobLease>, RuntimeJobsError> {
        Ok(self
            .coordinator
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)?)
    }

    pub fn acknowledge_completed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
    ) -> Result<(), RuntimeJobsError> {
        Ok(self.coordinator.acknowledge_completed(lease, now)?)
    }

    pub fn acknowledge_failed(
        &mut self,
        lease: &JobLease,
        now: JobInstant,
        reason: DeadLetterReason,
        error_message: impl Into<String>,
    ) -> Result<JobFailureDisposition, RuntimeJobsError> {
        Ok(self
            .coordinator
            .acknowledge_failed(lease, now, reason, error_message.into())?)
    }

    pub fn coordinator(&self) -> &JobsCoordinator {
        &self.coordinator
    }

    fn issue_job_id(&mut self, job_name: &str) -> Result<JobId, RuntimeJobsError> {
        self.next_job_sequence += 1;
        Ok(JobId::new(format!(
            "job:{}:{}",
            job_name, self.next_job_sequence
        ))?)
    }

    fn issue_event_id(&mut self, event_type: &str) -> Result<DomainEventId, RuntimeJobsError> {
        self.next_event_sequence += 1;
        Ok(DomainEventId::new(format!(
            "evt:{}:{}",
            event_type, self.next_event_sequence
        ))?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedReportExport {
    pub plan: ReportExportPlan,
    pub queued_job_id: JobId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedBulkOperation {
    pub plan: BulkOperationPlan,
    pub queued_job_id: JobId,
}

#[derive(Debug, Clone)]
pub struct OpsHost {
    planner: OpsPlanner,
    jobs: JobsHost,
}

impl OpsHost {
    pub fn planner(&self) -> &OpsPlanner {
        &self.planner
    }

    pub fn jobs(&self) -> &JobsHost {
        &self.jobs
    }

    pub fn jobs_mut(&mut self) -> &mut JobsHost {
        &mut self.jobs
    }

    pub fn queue_report_export(
        &mut self,
        request: ReportExportRequest,
    ) -> Result<QueuedReportExport, RuntimeOpsError> {
        let requested_at = request.requested_at;
        let plan = self.planner.plan_report_export(request)?;
        let queued_job_id = self.jobs.enqueue_spec(plan.job.clone(), requested_at)?;

        Ok(QueuedReportExport {
            plan,
            queued_job_id,
        })
    }

    pub fn queue_bulk_operation(
        &mut self,
        request: BulkOperationRequest,
    ) -> Result<QueuedBulkOperation, RuntimeOpsError> {
        let requested_at = request.requested_at;
        let plan = self.planner.plan_bulk_operation(request)?;
        let queued_job_id = self.jobs.enqueue_spec(plan.job.clone(), requested_at)?;

        Ok(QueuedBulkOperation {
            plan,
            queued_job_id,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeTlsError {
    #[error(transparent)]
    Tls(#[from] TlsModelError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsStatusSnapshot {
    pub customer_app: String,
    pub mode: davenda_config::TlsMode,
    pub edge_mode: EdgeMode,
    pub provider: Option<CertificateProviderKind>,
    pub inventory: CertificateInventory,
    pub queued_renewals: Vec<RenewalPlan>,
    pub pending_challenges: Vec<ChallengeTicket>,
    pub hot_reload_events: Vec<HotReloadEvent>,
}

#[derive(Debug, Clone)]
pub struct TlsHost {
    pub customer_app: String,
    pub runtime: TlsRuntimeServices,
    automation: TlsAutomationRuntime,
}

impl TlsHost {
    pub fn status(&self) -> TlsStatusSnapshot {
        TlsStatusSnapshot {
            customer_app: self.customer_app.clone(),
            mode: self.runtime.mode,
            edge_mode: self.runtime.edge_mode,
            provider: self.runtime.provider,
            inventory: self.automation.inventory().clone(),
            queued_renewals: self.automation.renewal_queue().to_vec(),
            pending_challenges: self.automation.pending_challenges().to_vec(),
            hot_reload_events: self.automation.hot_reload_events().to_vec(),
        }
    }

    pub fn issue_for_bindings(
        &self,
        bindings: Vec<HostnameBinding>,
    ) -> Result<IssuancePlan, RuntimeTlsError> {
        Ok(self.runtime.planner().issue_for_bindings(bindings)?)
    }

    pub fn import_certificate(&mut self, record: CertificateRecord) -> Result<(), RuntimeTlsError> {
        Ok(self.automation.import_certificate(record)?)
    }

    pub fn queue_renewal(
        &mut self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, RuntimeTlsError> {
        Ok(self.automation.queue_renewal(certificate_id, now)?)
    }

    pub fn begin_renewal(
        &mut self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, RuntimeTlsError> {
        Ok(self
            .automation
            .begin_renewal(certificate_id, replacement_certificate_id)?)
    }

    pub fn fail_renewal(
        &mut self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, RuntimeTlsError> {
        Ok(self.automation.fail_renewal(certificate_id)?)
    }

    pub fn activate_replacement(
        &mut self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, RuntimeTlsError> {
        Ok(self
            .automation
            .activate_replacement(certificate_id, replacement)?)
    }

    pub fn automation(&self) -> &TlsAutomationRuntime {
        &self.automation
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeCacheError {
    #[error(transparent)]
    Cache(#[from] CacheModelError),
}

#[derive(Debug, Clone)]
pub struct CacheHost {
    pub customer_app: String,
    pub namespace: CacheNamespace,
    pub planner: CachePlanner,
    runtime: CacheRuntime,
}

impl CacheHost {
    pub fn lookup_execution(
        &mut self,
        execution: &RequestExecution,
        now: CacheInstant,
    ) -> Option<CacheLookup> {
        execution
            .cache_plan
            .plan
            .application()
            .map(|plan| self.runtime.lookup(plan.key(), now))
    }

    pub fn begin_fill(
        &mut self,
        execution: &RequestExecution,
        holder: impl Into<String>,
    ) -> Option<FillDecision> {
        execution.cache_plan.plan.application().map(|plan| {
            self.runtime
                .begin_fill(plan.key(), plan.coalescing(), holder)
        })
    }

    pub fn complete_fill(&mut self, decision: &FillDecision) -> Result<(), RuntimeCacheError> {
        match decision {
            FillDecision::Start(lease) => Ok(self.runtime.complete_fill(lease)?),
            FillDecision::Coalesced { .. } | FillDecision::Uncoalesced => Ok(()),
        }
    }

    pub fn store_execution(
        &mut self,
        execution: &RequestExecution,
        value: impl Into<String>,
        now: CacheInstant,
    ) -> Option<CacheKey> {
        execution.cache_plan.plan.application().map(|plan| {
            self.runtime.insert(plan, value, now);
            plan.key().clone()
        })
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.runtime.invalidate(tags)
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.runtime.metrics()
    }
}

impl RuntimePlan {
    pub fn jobs_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<JobsHost, RuntimeJobsError> {
        let scheduler_node_id =
            validate_runtime_identifier("scheduler_node_id", scheduler_node_id.into())?;

        Ok(JobsHost {
            customer_app: self.config.app.name.clone(),
            scheduler_node_id,
            runtime: self.jobs.clone(),
            queue_topology: self.jobs.describe().clone(),
            registered_jobs: self.registered_runtime_jobs.clone(),
            registered_event_subscriptions: self.registered_runtime_event_subscriptions.clone(),
            jobs_domain: self.jobs_domain.clone(),
            coordinator: self.jobs.coordinator(),
            next_job_sequence: 0,
            next_event_sequence: 0,
        })
    }

    pub fn ops_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<OpsHost, RuntimeOpsError> {
        Ok(OpsHost {
            planner: OpsPlanner::new(self.jobs.clone(), self.ops_catalog.clone())?,
            jobs: self.jobs_host(scheduler_node_id)?,
        })
    }

    pub fn cache_host(&self) -> Result<CacheHost, RuntimeCacheError> {
        let namespace = self.cache_namespace()?;
        Ok(CacheHost {
            customer_app: self.config.app.name.clone(),
            namespace,
            planner: self.cache_planner,
            runtime: self.cache_planner.runtime(),
        })
    }

    pub fn tls_host(&self) -> TlsHost {
        TlsHost {
            customer_app: self.config.app.name.clone(),
            runtime: self.tls.clone(),
            automation: self.tls.automation(),
        }
    }

    fn cache_namespace(&self) -> Result<CacheNamespace, CacheModelError> {
        CacheNamespace::new(format!("customer-app:{}", self.config.app.name))
    }

    pub fn execute_request(
        &self,
        request: RequestInput,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<RequestExecution, RequestExecutionError> {
        let matched = self
            .http
            .resolve_match(&self.config, request.method, &request.host, &request.path)
            .ok_or_else(|| RequestExecutionError::RouteNotFound {
                method: request.method,
                host: request.host.clone(),
                path: request.path.clone(),
            })?;

        let trace = RequestTraceContext {
            request_id: request.request_id.clone().unwrap_or_else(|| {
                format!(
                    "req:{:?}:{}:{}",
                    request.method, request.host, matched.resolved.route_name
                )
            }),
            transport_scheme: request
                .forwarded_proto
                .clone()
                .unwrap_or_else(|| request.scheme.clone()),
        };

        let mut resolved_from_cookie = false;
        let session_id = if let Some(session_id) = request.session_id.clone() {
            Some(session_id)
        } else if let Some(cookie) = request.session_cookie.as_deref() {
            resolved_from_cookie = true;
            Some(self.verify_session_cookie(cookie_secret, cookie)?)
        } else {
            None
        };

        let session = SessionContext {
            session_id: session_id.clone(),
            resolved_from_cookie,
        };
        let principal = PrincipalContext {
            principal_id: request.principal_id.clone(),
            granted_capabilities: request.granted_capabilities.clone(),
        };

        self.enforce_maintenance_mode(&matched.route, request.method, &request)?;
        self.enforce_feature_flags(&matched.route)?;
        self.enforce_route_auth(&matched.resolved, &session, &principal)?;
        self.enforce_browser_policy(
            &matched.route,
            &matched.resolved,
            request.method,
            request.csrf_action.as_deref(),
            request.csrf_token.as_deref(),
            &session,
            csrf_secret,
        )?;
        let response = self
            .handlers
            .get(&matched.resolved.route_name)
            .cloned()
            .map(|handler| handler.response)
            .ok_or_else(|| RequestExecutionError::HandlerNotRegistered {
                route: matched.resolved.route_name.clone(),
            })?;
        let cache = cache_disposition_for_route(request.method, &matched.resolved.auth, &session);
        let cache_plan = build_execution_cache_plan(
            self,
            &request,
            &matched.route,
            &matched.resolved,
            &session,
            &principal,
            cache,
        )?;

        Ok(RequestExecution {
            customer_app: self.config.app.name.clone(),
            route: matched.resolved.clone(),
            route_area: matched.route.area,
            locale: matched
                .resolved
                .locale
                .clone()
                .unwrap_or_else(|| self.config.i18n.default_locale.clone()),
            trace,
            session: session.clone(),
            principal,
            cache,
            cache_plan,
            middleware: self.http.middleware.clone(),
            response,
        })
    }

    fn verify_session_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<String, RequestExecutionError> {
        let signer = CookieSigner::new(self.browser.sessions.session_cookie.clone());
        signer
            .verify(cookie_secret, cookie)
            .map_err(|error| RequestExecutionError::InvalidSessionCookie(error.to_string()))
    }

    fn enforce_route_auth(
        &self,
        route: &ResolvedRoute,
        session: &SessionContext,
        principal: &PrincipalContext,
    ) -> Result<(), RequestExecutionError> {
        match route.auth {
            RouteAuthGate::Public => Ok(()),
            RouteAuthGate::Session => {
                if session.session_id.is_some() {
                    Ok(())
                } else {
                    Err(RequestExecutionError::SessionRequired {
                        route: route.route_name.clone(),
                    })
                }
            }
            RouteAuthGate::Capability(capability) => {
                if session.session_id.is_none() {
                    return Err(RequestExecutionError::SessionRequired {
                        route: route.route_name.clone(),
                    });
                }

                if principal.granted_capabilities.contains(&capability) {
                    Ok(())
                } else {
                    Err(RequestExecutionError::CapabilityRequired {
                        route: route.route_name.clone(),
                        capability,
                    })
                }
            }
        }
    }

    fn enforce_feature_flags(&self, route: &RouteDefinition) -> Result<(), RequestExecutionError> {
        let Some(feature_flag) = route.feature_flag.as_deref() else {
            return Ok(());
        };

        let Some(feature_flag_id) = FeatureFlagId::new(feature_flag.to_string()).ok() else {
            return Err(RequestExecutionError::FeatureFlagDisabled {
                route: route.name.clone(),
                feature_flag: feature_flag.to_string(),
            });
        };
        let context = FeatureFlagContext {
            environment: self.config.app.environment,
            customer_app: CustomerAppId::new(self.config.app.name.clone()).ok(),
            site: None,
            brand: None,
            cohorts: BTreeSet::new(),
        };

        match self.observability.flags.get(&feature_flag_id) {
            Some(flag) if flag.enabled_for(&context) => Ok(()),
            _ => Err(RequestExecutionError::FeatureFlagDisabled {
                route: route.name.clone(),
                feature_flag: feature_flag.to_string(),
            }),
        }
    }

    fn enforce_maintenance_mode(
        &self,
        route: &RouteDefinition,
        method: HttpMethod,
        request: &RequestInput,
    ) -> Result<(), RequestExecutionError> {
        let customer_app = CustomerAppId::new(self.config.app.name.clone()).ok();
        let blocked = self.observability.maintenance.blocks_request(
            customer_app.as_ref(),
            method.is_state_changing(),
            request.maintenance_bypass_token.as_deref(),
        );

        if blocked {
            Err(RequestExecutionError::MaintenanceMode {
                route: route.name.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn enforce_browser_policy(
        &self,
        route: &RouteDefinition,
        resolved: &ResolvedRoute,
        method: HttpMethod,
        csrf_action: Option<&str>,
        csrf_token: Option<&str>,
        session: &SessionContext,
        csrf_secret: &[u8],
    ) -> Result<(), RequestExecutionError> {
        let requires_csrf = method.is_state_changing()
            && route.area != RouteArea::Api
            && self.browser.csrf.enabled
            && !matches!(resolved.auth, RouteAuthGate::Public);

        if !requires_csrf {
            return Ok(());
        }

        let session_id = session.session_id.as_deref().ok_or_else(|| {
            RequestExecutionError::MissingSessionForCsrf {
                route: resolved.route_name.clone(),
            }
        })?;
        let token = csrf_token.ok_or_else(|| RequestExecutionError::MissingCsrfToken {
            route: resolved.route_name.clone(),
        })?;
        let action = csrf_action.unwrap_or(&resolved.route_name);
        let valid = self
            .browser
            .csrf
            .verify_token(csrf_secret, session_id, action, token)
            .map_err(|_| RequestExecutionError::InvalidCsrfToken {
                route: resolved.route_name.clone(),
            })?;

        if valid {
            Ok(())
        } else {
            Err(RequestExecutionError::InvalidCsrfToken {
                route: resolved.route_name.clone(),
            })
        }
    }
}

fn cache_disposition_for_route(
    method: HttpMethod,
    auth: &RouteAuthGate,
    session: &SessionContext,
) -> CacheDisposition {
    if method.is_state_changing() {
        return CacheDisposition::Uncacheable;
    }

    match auth {
        RouteAuthGate::Public if session.session_id.is_none() => CacheDisposition::Public,
        _ => CacheDisposition::Private,
    }
}

fn build_execution_cache_plan(
    runtime: &RuntimePlan,
    request: &RequestInput,
    route: &RouteDefinition,
    resolved: &ResolvedRoute,
    session: &SessionContext,
    principal: &PrincipalContext,
    disposition: CacheDisposition,
) -> Result<ExecutedCachePlan, CacheModelError> {
    let scope = cache_scope_for_request(request, resolved, session, principal, disposition)?;
    let tags = cache_tags_for_request(runtime, route, resolved, request)?;
    let validators = cache_validators_for_request(request, resolved, session, principal)?;
    let freshness = cache_freshness_for_request(route, request.method, disposition);
    let http_policy = HttpCachePolicy::new(scope.clone(), freshness, validators, tags.clone())?;
    let mut cache_request = CachePlanRequest::new(
        runtime.cache_namespace()?,
        request.path.clone(),
        http_policy,
    )?;

    if let Some(freshness) = freshness.filter(|_| disposition != CacheDisposition::Uncacheable) {
        cache_request = cache_request
            .with_application_policy(ApplicationCachePolicy::new(scope, freshness, tags)?);
    }

    let plan = runtime.cache_planner.plan(cache_request)?;
    let headers = cache_headers_from_plan(&plan);

    Ok(ExecutedCachePlan { plan, headers })
}

fn cache_scope_for_request(
    request: &RequestInput,
    resolved: &ResolvedRoute,
    session: &SessionContext,
    principal: &PrincipalContext,
    disposition: CacheDisposition,
) -> Result<CacheScope, CacheModelError> {
    let mut scope = match disposition {
        CacheDisposition::Public => CacheScope::public(),
        CacheDisposition::Private => CacheScope::private(),
        CacheDisposition::Uncacheable => CacheScope::no_store(),
    }
    .with_site(request.host.clone())?;

    if let Some(locale) = resolved.locale.as_deref() {
        scope = scope.with_locale(locale.to_string())?;
    }

    if disposition == CacheDisposition::Private {
        if let Some(principal_id) = principal.principal_id.as_deref() {
            scope = scope.with_user(principal_id.to_string())?;
        } else if let Some(session_id) = session.session_id.as_deref() {
            scope = scope.with_session(session_id.to_string())?;
        }
    }

    Ok(scope)
}

fn cache_tags_for_request(
    runtime: &RuntimePlan,
    route: &RouteDefinition,
    resolved: &ResolvedRoute,
    request: &RequestInput,
) -> Result<InvalidationSet, CacheModelError> {
    let mut tags = InvalidationSet::new();
    tags.insert(InvalidationTag::new(format!(
        "customer_app:{}",
        runtime.config.app.name
    ))?);
    tags.insert(InvalidationTag::new(format!(
        "route:{}",
        resolved.route_name
    ))?);
    tags.insert(InvalidationTag::new(format!("path:{}", request.path))?);

    if let Some(module) = route.module.as_deref() {
        tags.insert(InvalidationTag::new(format!("module:{module}"))?);
    }

    if let Some(locale) = resolved.locale.as_deref() {
        tags.insert(InvalidationTag::new(format!("locale:{locale}"))?);
    }

    Ok(tags)
}

fn cache_validators_for_request(
    request: &RequestInput,
    resolved: &ResolvedRoute,
    session: &SessionContext,
    principal: &PrincipalContext,
) -> Result<ResponseValidators, CacheModelError> {
    let mut parts = vec![
        "etag".to_string(),
        resolved.route_name.clone(),
        request.host.clone(),
        request.path.clone(),
    ];

    if let Some(locale) = resolved.locale.as_deref() {
        parts.push(format!("locale:{locale}"));
    }
    if let Some(principal_id) = principal.principal_id.as_deref() {
        parts.push(format!("user:{principal_id}"));
    } else if let Some(session_id) = session.session_id.as_deref() {
        parts.push(format!("session:{session_id}"));
    }

    Ok(ResponseValidators {
        etag: Some(EntityTag::new(parts.join(":"))?),
        last_modified_unix_seconds: None,
    })
}

fn cache_freshness_for_request(
    route: &RouteDefinition,
    method: HttpMethod,
    disposition: CacheDisposition,
) -> Option<FreshnessPolicy> {
    if method.is_state_changing() || disposition == CacheDisposition::Uncacheable {
        return None;
    }

    match disposition {
        CacheDisposition::Public => Some(
            FreshnessPolicy::new(Duration::from_secs(300), Some(Duration::from_secs(30)))
                .expect("constant public freshness is valid"),
        ),
        CacheDisposition::Private if route.area == RouteArea::Account => Some(
            FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(30)))
                .expect("constant account freshness is valid"),
        ),
        CacheDisposition::Private => Some(
            FreshnessPolicy::new(Duration::from_secs(30), Some(Duration::from_secs(15)))
                .expect("constant private freshness is valid"),
        ),
        CacheDisposition::Uncacheable => None,
    }
}

fn cache_headers_from_plan(plan: &CachePlan) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(
        "Cache-Control".to_string(),
        plan.http().cache_control().to_string(),
    );

    if let Some(variation) = plan.http().variation() {
        headers.insert(
            "X-Davenda-Variation-Key".to_string(),
            variation.as_str().to_string(),
        );
    }

    if let Some(etag) = plan.http().validators().etag.as_ref() {
        headers.insert("ETag".to_string(), etag.as_str().to_string());
    }

    if let Some(surrogate_tags) = plan.http().surrogate_tags().header_value() {
        headers.insert("Surrogate-Key".to_string(), surrogate_tags);
    }

    headers
}

fn module_http_contributions(
    manifests: &[ModuleManifest],
) -> Result<(Vec<RouteDefinition>, Vec<HandlerDefinition>), RouteBuildError> {
    let mut routes = Vec::new();
    let mut handlers = Vec::new();

    for manifest in manifests {
        for surface in &manifest.http_surfaces {
            routes.push(route_definition_from_surface(&manifest.name, surface)?);
            handlers.push(handler_definition_from_surface(surface)?);
        }
    }

    Ok((routes, handlers))
}

fn route_definition_from_surface(
    module: &str,
    surface: &HttpSurfaceContribution,
) -> Result<RouteDefinition, RouteBuildError> {
    let mut route = RouteDefinition::new(
        surface.name.clone(),
        http_method_from_surface(surface.method),
        surface.path.clone(),
    )?
    .from_module(module.to_string());

    route = match surface.area {
        HttpSurfaceArea::Public => route,
        HttpSurfaceArea::Account => route.with_area(RouteArea::Account),
        HttpSurfaceArea::Admin => route.with_area(RouteArea::Admin),
        HttpSurfaceArea::Api => route.with_area(RouteArea::Api),
        HttpSurfaceArea::Fragment => route.with_area(RouteArea::Fragment),
    };

    if surface.localized {
        route = route.localized();
    }

    route = match surface.capability {
        Some(capability) => route.requiring_capability(capability),
        None if surface.area == HttpSurfaceArea::Account => route.requiring_session(),
        None if surface.area == HttpSurfaceArea::Admin => route.requiring_session(),
        None => route,
    };

    Ok(route)
}

fn handler_definition_from_surface(
    surface: &HttpSurfaceContribution,
) -> Result<HandlerDefinition, RouteBuildError> {
    match &surface.response {
        HttpResponseContract::Page { template, status } => {
            let mut handler = HandlerDefinition::page(surface.name.clone(), template.clone())?;
            if let HandlerResponse::Page(page) = &mut handler.response {
                page.status = *status;
            }
            Ok(handler)
        }
        HttpResponseContract::Fragment {
            template,
            fragment_id,
        } => {
            HandlerDefinition::fragment(surface.name.clone(), template.clone(), fragment_id.clone())
        }
        HttpResponseContract::Redirect { location, status } => {
            let mut handler = HandlerDefinition::redirect(surface.name.clone(), location.clone())?;
            if let HandlerResponse::Redirect(redirect) = &mut handler.response {
                redirect.status = *status;
            }
            Ok(handler)
        }
        HttpResponseContract::Json { status, payload } => {
            let mut handler = HandlerDefinition::json(surface.name.clone(), payload.clone())?;
            if let HandlerResponse::Json(json) = &mut handler.response {
                json.status = *status;
            }
            Ok(handler)
        }
        HttpResponseContract::File {
            logical_path,
            content_type,
            delivery_mode,
        } => HandlerDefinition::file(
            surface.name.clone(),
            logical_path.clone(),
            content_type.clone(),
            file_delivery_mode_from_surface(*delivery_mode),
        ),
    }
}

fn http_method_from_surface(method: HttpSurfaceMethod) -> HttpMethod {
    match method {
        HttpSurfaceMethod::Get => HttpMethod::Get,
        HttpSurfaceMethod::Head => HttpMethod::Head,
        HttpSurfaceMethod::Post => HttpMethod::Post,
        HttpSurfaceMethod::Put => HttpMethod::Put,
        HttpSurfaceMethod::Patch => HttpMethod::Patch,
        HttpSurfaceMethod::Delete => HttpMethod::Delete,
    }
}

fn file_delivery_mode_from_surface(mode: HttpFileDeliveryMode) -> FileDeliveryMode {
    match mode {
        HttpFileDeliveryMode::PublicCdn => FileDeliveryMode::PublicCdn,
        HttpFileDeliveryMode::SignedUrl => FileDeliveryMode::SignedUrl,
        HttpFileDeliveryMode::AppProxy => FileDeliveryMode::AppProxy,
        HttpFileDeliveryMode::LocalOnly => FileDeliveryMode::LocalOnly,
    }
}

fn match_route_path(pattern: &str, actual: &str) -> Option<BTreeMap<String, String>> {
    let pattern_segments = path_segments(pattern);
    let actual_segments = path_segments(actual);
    if pattern_segments.len() != actual_segments.len() {
        return None;
    }

    let mut params = BTreeMap::new();
    for (pattern_segment, actual_segment) in pattern_segments.iter().zip(actual_segments.iter()) {
        if pattern_segment.starts_with('{')
            && pattern_segment.ends_with('}')
            && pattern_segment.len() > 2
        {
            params.insert(
                pattern_segment[1..pattern_segment.len() - 1].to_string(),
                (*actual_segment).to_string(),
            );
        } else if pattern_segment != actual_segment {
            return None;
        }
    }

    Some(params)
}

fn path_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn render_route_path(
    pattern: &str,
    params: &BTreeMap<String, String>,
    route_name: &str,
) -> Result<String, RouteUrlError> {
    let rendered_segments = path_segments(pattern)
        .into_iter()
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2 {
                let parameter = &segment[1..segment.len() - 1];
                params
                    .get(parameter)
                    .cloned()
                    .ok_or_else(|| RouteUrlError::MissingRouteParameter {
                        route: route_name.to_string(),
                        parameter: parameter.to_string(),
                    })
            } else {
                Ok(segment.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if rendered_segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", rendered_segments.join("/")))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeJobsError {
    #[error(transparent)]
    Jobs(#[from] JobsModelError),
    #[error("runtime value `{field}` cannot be empty")]
    EmptyValue { field: &'static str },
    #[error("job `{job}` is not declared by the runtime")]
    UnknownJob { job: String },
    #[error("job `{job}` must be dispatched through a domain event")]
    DomainEventJobRequiresEventDispatch { job: String },
    #[error("scheduled job `{job}` requires a scheduled execution instant")]
    ScheduledJobRequiresSchedule { job: String },
    #[error("job `{job}` uses trigger `{trigger:?}` and cannot be scheduled explicitly")]
    UnexpectedSchedule {
        job: String,
        trigger: JobTriggerKind,
    },
    #[error("job `{job}` requires an explicit idempotency key")]
    MissingIdempotencyKey { job: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeOpsError {
    #[error(transparent)]
    Ops(#[from] OpsModelError),
    #[error(transparent)]
    Jobs(#[from] RuntimeJobsError),
}

#[derive(Debug, Error)]
pub enum RuntimeBuildError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error(transparent)]
    Capability(#[from] CapabilityValidationError),
    #[error(transparent)]
    ModuleInstallation(#[from] ModuleInstallationError),
    #[error(transparent)]
    Data(#[from] DataModelError),
    #[error(transparent)]
    Route(#[from] RouteBuildError),
    #[error(transparent)]
    Observability(#[from] ObservabilityError),
    #[error(transparent)]
    Wasm(#[from] WasmModelError),
    #[error(transparent)]
    Jobs(#[from] JobsModelError),
    #[error(transparent)]
    Ops(#[from] OpsModelError),
    #[error("configured auth package `{configured}` does not match loaded package `{actual}`")]
    AuthPackageMismatch { configured: String, actual: String },
    #[error(
        "installed extension `{extension_id}` targets customer app `{actual}` but runtime config is `{configured}`"
    )]
    ExtensionCustomerAppMismatch {
        extension_id: String,
        configured: String,
        actual: String,
    },
    #[error("handler `{route}` is registered more than once")]
    DuplicateHandler { route: String },
    #[error("handler `{route}` does not match a registered route")]
    UnknownHandlerRoute { route: String },
    #[error(
        "extension slot `{surface}` for `{kind:?}` is declared by both `{first_module}` and `{second_module}`"
    )]
    DuplicateExtensionSlot {
        kind: ExtensionPointKind,
        surface: String,
        first_module: String,
        second_module: String,
    },
    #[error(
        "installed extension `{extension_id}` handler `{handler_id}` targets `{point}` surface `{surface}` without a declared slot"
    )]
    UnknownExtensionSlot {
        extension_id: String,
        handler_id: String,
        point: ExtensionPointKind,
        surface: String,
    },
    #[error(
        "job `{job}` is declared by both `{first_module}` and `{second_module}`; runtime job names must be unique"
    )]
    DuplicateRuntimeJobName {
        job: String,
        first_module: String,
        second_module: String,
    },
    #[error("event subscription `{event}` in module `{module}` must target a declared job")]
    EventSubscriptionMissingJob { module: String, event: String },
    #[error("event subscription `{event}` in module `{module}` targets unknown job `{job}`")]
    UnknownEventSubscriptionJob {
        module: String,
        event: String,
        job: String,
    },
    #[error(
        "event subscription `{event}` in module `{module}` targets job `{job}` with trigger `{trigger:?}`; domain-event subscriptions must target domain-event jobs"
    )]
    EventSubscriptionTriggerMismatch {
        module: String,
        event: String,
        job: String,
        trigger: JobTriggerKind,
    },
}

fn build_runtime_jobs_domain(
    runtime: &JobsRuntimeServices,
    module_jobs: &[RegisteredModuleJob],
    module_event_subscriptions: &[RegisteredEventSubscription],
) -> Result<
    (
        Vec<RuntimeJobDefinition>,
        Vec<RuntimeEventSubscriptionDefinition>,
        JobsDomain,
    ),
    RuntimeBuildError,
> {
    let mut jobs_by_name = BTreeMap::<String, RuntimeJobDefinition>::new();

    for registered in module_jobs {
        let queue = queue_for_job_trigger(runtime, registered.job.trigger);
        let retry_policy = retry_policy_for_job(runtime, &queue, &registered.job);
        let job = RuntimeJobDefinition {
            module: registered.module.clone(),
            contract: registered.job.clone(),
            queue,
            retry_policy,
        };

        if let Some(existing) = jobs_by_name.insert(job.contract.name.clone(), job.clone()) {
            return Err(RuntimeBuildError::DuplicateRuntimeJobName {
                job: job.contract.name,
                first_module: existing.module,
                second_module: job.module,
            });
        }
    }

    let mut domain = JobsDomain::new(runtime.clone());
    let mut subscriptions_by_handler = BTreeMap::<String, Vec<EventSubscriptionMetadata>>::new();
    let mut resolved_subscriptions = Vec::new();

    for registered in module_event_subscriptions {
        let Some(job_name) = registered.subscription.job.clone() else {
            return Err(RuntimeBuildError::EventSubscriptionMissingJob {
                module: registered.module.clone(),
                event: registered.subscription.event.clone(),
            });
        };
        let Some(job) = jobs_by_name.get(&job_name) else {
            return Err(RuntimeBuildError::UnknownEventSubscriptionJob {
                module: registered.module.clone(),
                event: registered.subscription.event.clone(),
                job: job_name,
            });
        };

        let event_type = DomainEventType::new(registered.subscription.event.clone())?;
        let subscription_id = EventSubscriptionId::new(format!(
            "{}:{}:{}",
            registered.module, registered.subscription.event, job.contract.name
        ))?;
        let handler_id = EventHandlerId::new(job.contract.name.clone())?;
        let reaction_queue = runtime.describe().domain_events_queue.clone();
        let reaction_retry_policy =
            retry_policy_for_contract_shape(runtime, &reaction_queue, job.contract.idempotent);
        let mut metadata = EventSubscriptionMetadata::new(
            subscription_id.clone(),
            event_type.clone(),
            reaction_queue.clone(),
            handler_id.clone(),
            reaction_retry_policy.clone(),
        );

        if reaction_retry_policy.is_retrying() {
            metadata = metadata.with_idempotency_key(IdempotencyKey::new(format!(
                "subscription:{}",
                subscription_id.as_str()
            ))?);
        }

        metadata = metadata.with_description(registered.subscription.description.clone())?;
        subscriptions_by_handler
            .entry(job.contract.name.clone())
            .or_default()
            .push(metadata.clone());
        resolved_subscriptions.push(RuntimeEventSubscriptionDefinition {
            module: registered.module.clone(),
            event_type,
            subscription_id,
            handler_id,
            job_name: job.contract.name.clone(),
            reaction_queue,
            retry_policy: reaction_retry_policy,
            target_trigger: job.contract.trigger,
            target_queue: job.queue.clone(),
            description: registered.subscription.description.clone(),
        });
        domain = domain.add_subscription(metadata);
    }

    let mut resolved_jobs = jobs_by_name.into_values().collect::<Vec<_>>();
    resolved_jobs.sort_by(|left, right| left.contract.name.cmp(&right.contract.name));
    resolved_subscriptions.sort_by(|left, right| left.subscription_id.cmp(&right.subscription_id));

    for (job_name, subscriptions) in &subscriptions_by_handler {
        let handler_id = EventHandlerId::new(job_name.clone())?;
        let mut handler = EventHandlerMetadata::new(
            handler_id,
            job_name.clone(),
            runtime.describe().domain_events_queue.clone(),
            RetryPolicy::default(),
        )?;

        for subscription in subscriptions {
            handler = handler.add_subscription(subscription.clone());
        }

        domain = domain.add_handler(handler);
    }

    domain.validate()?;

    Ok((resolved_jobs, resolved_subscriptions, domain))
}

fn queue_for_job_trigger(runtime: &JobsRuntimeServices, trigger: JobTriggerKind) -> JobQueueName {
    match trigger {
        JobTriggerKind::Scheduled => runtime.describe().scheduled_queue.clone(),
        JobTriggerKind::DomainEvent => runtime.describe().domain_events_queue.clone(),
        JobTriggerKind::Operator | JobTriggerKind::Webhook | JobTriggerKind::InlineFollowup => {
            runtime.describe().work_queue.clone()
        }
    }
}

fn retry_policy_for_job(
    runtime: &JobsRuntimeServices,
    queue: &JobQueueName,
    contract: &JobContract,
) -> RetryPolicy {
    retry_policy_for_contract_shape(runtime, queue, contract.idempotent)
}

fn retry_policy_for_contract_shape(
    runtime: &JobsRuntimeServices,
    queue: &JobQueueName,
    idempotent: bool,
) -> RetryPolicy {
    if idempotent {
        runtime
            .describe()
            .queue(queue)
            .map(|definition| definition.retry_policy.clone())
            .unwrap_or_default()
    } else {
        RetryPolicy::default()
    }
}

fn validate_runtime_identifier(
    field: &'static str,
    value: String,
) -> Result<String, RuntimeJobsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RuntimeJobsError::EmptyValue { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn collect_extension_slots(
    manifests: &[ModuleManifest],
) -> Result<Vec<RegisteredExtensionSlot>, RuntimeBuildError> {
    let mut slots = Vec::new();
    let mut seen = BTreeMap::<(ExtensionPointKind, String), String>::new();

    for manifest in manifests {
        for slot in &manifest.extension_slots {
            let kind = extension_point_kind_for_slot(slot);
            let key = (kind, slot.surface.clone());
            if let Some(existing_module) = seen.insert(key.clone(), manifest.name.clone()) {
                return Err(RuntimeBuildError::DuplicateExtensionSlot {
                    kind,
                    surface: key.1,
                    first_module: existing_module,
                    second_module: manifest.name.clone(),
                });
            }

            slots.push(RegisteredExtensionSlot {
                module: manifest.name.clone(),
                kind,
                surface: slot.surface.clone(),
                description: slot.description.clone(),
            });
        }
    }

    Ok(slots)
}

fn validate_extension_handler_slot(
    handler: &davenda_wasm::RegisteredExtensionHandler,
    slots: &[RegisteredExtensionSlot],
) -> Result<(), RuntimeBuildError> {
    if slots
        .iter()
        .any(|slot| slot.kind == handler.point && slot.surface == handler.surface)
    {
        Ok(())
    } else {
        Err(RuntimeBuildError::UnknownExtensionSlot {
            extension_id: handler.extension_id.to_string(),
            handler_id: handler.handler_id.to_string(),
            point: handler.point,
            surface: handler.surface.clone(),
        })
    }
}

fn extension_point_kind_for_slot(
    slot: &davenda_core::ExtensionSlotDescriptor,
) -> ExtensionPointKind {
    match slot.kind {
        davenda_core::ExtensionSlotKind::Page => ExtensionPointKind::Page,
        davenda_core::ExtensionSlotKind::Api => ExtensionPointKind::Api,
        davenda_core::ExtensionSlotKind::Job => ExtensionPointKind::Job,
        davenda_core::ExtensionSlotKind::ScheduledJob => ExtensionPointKind::ScheduledJob,
        davenda_core::ExtensionSlotKind::Webhook => ExtensionPointKind::Webhook,
        davenda_core::ExtensionSlotKind::AdminWidget => ExtensionPointKind::AdminWidget,
        davenda_core::ExtensionSlotKind::RenderHook => ExtensionPointKind::RenderHook,
    }
}

fn build_handler_registry(
    routes: &[RouteDefinition],
    handlers: Vec<HandlerDefinition>,
) -> Result<BTreeMap<String, HandlerDefinition>, RuntimeBuildError> {
    let known_routes = routes
        .iter()
        .map(|route| route.name.as_str())
        .collect::<HashSet<_>>();
    let mut registry = BTreeMap::new();

    for handler in handlers {
        if !known_routes.contains(handler.route_name.as_str()) {
            return Err(RuntimeBuildError::UnknownHandlerRoute {
                route: handler.route_name,
            });
        }

        if registry
            .insert(handler.route_name.clone(), handler.clone())
            .is_some()
        {
            return Err(RuntimeBuildError::DuplicateHandler {
                route: handler.route_name,
            });
        }
    }

    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_admin::AdminModule;
    use davenda_auth::{Capability, DefaultAuthModelPackage};
    use davenda_cache::CacheLookupState;
    use davenda_cache::DistributedCacheBackend;
    use davenda_cms::CmsModule;
    use davenda_commerce::CommerceModule;
    use davenda_core::CookieSigner;
    use davenda_events::EventsModule;
    use davenda_media::MediaModule;
    use davenda_memberships::MembershipsModule;
    use davenda_observability::{
        CustomerAppId as FlagCustomerAppId, FeatureFlag, MaintenanceAudience, MaintenanceImpact,
    };
    use davenda_ops::{
        BulkExecutionId, BulkOperationId, BulkOperationRequest, OpsModule, ReportExportId,
        ReportExportRequest, ReportId,
    };
    use davenda_template::TemplateNamespace;
    use davenda_tls::{
        CertificateFingerprint, CertificateId, CertificateProviderKind, CertificateRecord,
        CertificateStateStore, CertificateStatus, CloudflareEncryptionMode, CustomerAppId,
        Hostname, HostnameBinding, SecretMaterialRef, TlsInstant,
    };
    use davenda_wasm::{
        AdminWidgetExtensionPoint, ContractVersion, ExtensionInstallation, ExtensionManifest,
        ExtensionPoint, ExtensionPointKind, HandlerId, HandlerInstallation, HandlerManifest,
        HostCapabilityGrant, HostGrantSet, InstalledExtension, ResourceLimits,
    };
    use std::time::Duration;

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
localized_routes = true

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

    fn installed_admin_widget_extension() -> InstalledExtension {
        InstalledExtension::install(
            ExtensionManifest::new(
                davenda_wasm::ExtensionId::new("admin.waitlist").unwrap(),
                "Waitlist Dashboard Widgets",
                ContractVersion::new(1, 0, 0),
                ContractVersion::new(1, 0, 0),
                ResourceLimits::baseline_for(ExtensionPointKind::AdminWidget),
                vec![
                    HandlerManifest::new(
                        HandlerId::new("waitlist-summary").unwrap(),
                        "exports.waitlist_summary",
                        ExtensionPoint::AdminWidget(
                            AdminWidgetExtensionPoint::new("admin.dashboard.summary").unwrap(),
                        ),
                        HostGrantSet::from_grants([
                            HostCapabilityGrant::AuthCheck,
                            HostCapabilityGrant::DataRead {
                                resource: "events.waitlist".to_string(),
                            },
                        ]),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
            ExtensionInstallation::new(
                "showcase-events",
                vec![HandlerInstallation::new(
                    HandlerId::new("waitlist-summary").unwrap(),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::AuthCheck,
                        HostCapabilityGrant::DataRead {
                            resource: "events.waitlist".to_string(),
                        },
                    ]),
                )],
            )
            .unwrap(),
        )
        .unwrap()
    }

    #[derive(Debug)]
    struct StaticManifestModule {
        manifest: ModuleManifest,
    }

    impl StaticManifestModule {
        fn new(manifest: ModuleManifest) -> Self {
            Self { manifest }
        }
    }

    impl PlatformModule for StaticManifestModule {
        fn manifest(&self) -> ModuleManifest {
            self.manifest.clone()
        }

        fn register(
            &self,
            _registry: &mut davenda_core::ServiceRegistry,
        ) -> Result<(), RegistrationError> {
            Ok(())
        }
    }

    fn external_tls_config() -> String {
        VALID_CONFIG.replace(
            "mode = \"acme\"\nchallenge = \"dns-01\"\nprovider = \"cloudflare-dns\"",
            "mode = \"external\"",
        )
    }

    fn cloudflare_origin_tls_config() -> String {
        VALID_CONFIG.replace(
            "mode = \"acme\"\nchallenge = \"dns-01\"\nprovider = \"cloudflare-dns\"",
            "mode = \"cloudflare-origin\"\nprovider = \"cloudflare-origin-ca\"",
        )
    }

    fn active_certificate(id: &str, hostname: &str) -> CertificateRecord {
        CertificateRecord::new(
            CertificateId::new(id).unwrap(),
            CertificateProviderKind::Acme,
            CertificateStatus::Active,
            CertificateFingerprint::new(format!("sha256:{id}")).unwrap(),
            TlsInstant::from_unix_seconds(1_000),
            TlsInstant::from_unix_seconds(4_000_000),
            SecretMaterialRef::new(format!("secrets/tls/{id}")).unwrap(),
            CertificateStateStore::SharedSecrets,
        )
        .with_binding(HostnameBinding::new(
            Hostname::new(hostname).unwrap(),
            CustomerAppId::new("showcase-events").unwrap(),
        ))
    }

    #[test]
    fn runtime_builder_creates_a_runtime_plan() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("events.show", HttpMethod::Get, "/events")
                    .unwrap()
                    .localized()
                    .from_module("events"),
            )
            .with_route(
                RouteDefinition::new("admin.settings", HttpMethod::Get, "/admin/settings")
                    .unwrap()
                    .with_area(RouteArea::Admin)
                    .requiring_session(),
            )
            .with_module(AdminModule::new())
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(MembershipsModule::new())
            .with_module(EventsModule::new())
            .with_module(MediaModule::new())
            .build()
            .unwrap();

        assert_eq!(plan.auth_package_name, "platform-default-auth");
        assert_eq!(
            plan.cache_topology.l2(),
            Some(DistributedCacheBackend::Redis)
        );
        assert_eq!(plan.browser.sessions.session_cookie.name, "davenda_session");
        assert_eq!(plan.browser.csrf.field_name, "_csrf");
        assert!(
            plan.cli
                .registry
                .commands()
                .any(|command| command.path == vec!["tls".to_string(), "renew".to_string()])
        );
        assert_eq!(plan.data.driver, davenda_config::DatabaseDriver::Postgres);
        assert_eq!(plan.data.schema, "public");
        assert_eq!(plan.jobs.backend, davenda_config::JobBackend::Redis);
        assert_eq!(
            plan.jobs.topology.scheduled_queue.as_str(),
            "jobs.scheduled"
        );
        assert_eq!(plan.tls.mode, davenda_config::TlsMode::Acme);
        assert_eq!(
            plan.tls.provider.map(|provider| provider.to_string()),
            Some("cloudflare_dns".to_string())
        );
        assert!(plan.observability.telemetry.metrics_enabled);
        assert!(plan.observability.telemetry.trace.enabled);
        assert_eq!(
            plan.observability.readiness.overall_status(),
            davenda_observability::DependencyStatus::Healthy
        );
        assert_eq!(
            plan.http.middleware,
            vec![
                MiddlewareStage::TransportNormalization,
                MiddlewareStage::CustomerAppResolution,
                MiddlewareStage::TraceContext,
                MiddlewareStage::LocaleResolution,
                MiddlewareStage::SessionResolution,
                MiddlewareStage::BrowserPolicy,
                MiddlewareStage::ResponsePolicy,
            ]
        );
        assert_eq!(
            plan.http.resolve(
                &plan.config,
                HttpMethod::Get,
                "www.example.com",
                "/fr-FR/events"
            ),
            Some(ResolvedRoute {
                route_name: "events.show".to_string(),
                locale: Some("fr-FR".to_string()),
                auth: RouteAuthGate::Public,
                params: BTreeMap::new(),
            })
        );
        assert_eq!(
            plan.template
                .namespace_chain(Some(&TemplateNamespace::new("cms-pages").unwrap())),
            vec![
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateNamespace::new("cms-pages").unwrap(),
                TemplateNamespace::new("core").unwrap(),
            ]
        );
        assert_eq!(plan.wasm.extension_directory, "extensions");
        assert_eq!(
            plan.wasm
                .limits
                .for_point(ExtensionPointKind::Page)
                .max_runtime,
            Duration::from_millis(50)
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.admin.shell")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.cms.pages")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.commerce.checkout")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.memberships.entitlements")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.events.bookings")
        );
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.media.assets")
        );
        assert_eq!(plan.modules.len(), 6);
        assert_eq!(plan.modules[0].name, "admin");
        assert_eq!(plan.modules[1].name, "cms");
        assert_eq!(plan.modules[2].name, "commerce");
        assert_eq!(plan.modules[3].name, "memberships");
        assert_eq!(plan.modules[4].name, "events");
        assert_eq!(plan.modules[5].name, "media");
        assert!(
            plan.install_migrations
                .ordered_steps()
                .iter()
                .any(|step| step.owner == davenda_data::MigrationOwner::Module("cms".to_string()))
        );
        assert!(plan.install_migrations.ordered_steps().iter().any(|step| {
            step.owner == davenda_data::MigrationOwner::Module("memberships".to_string())
        }));
        assert!(
            plan.module_jobs
                .iter()
                .any(|registered| registered.job.name == "events.reminders")
        );
        assert!(plan.module_event_subscriptions.iter().any(|registered| {
            registered.subscription.event == "commerce.order.paid"
                && registered.module == "memberships"
        }));
        assert!(plan.registered_runtime_jobs.iter().any(|registered| {
            registered.contract.name == "events.reminders"
                && registered.queue == plan.jobs.topology.scheduled_queue
        }));
        assert!(
            plan.registered_runtime_event_subscriptions
                .iter()
                .any(|registered| {
                    registered.module == "memberships"
                        && registered.event_type.as_str() == "commerce.order.paid"
                        && registered.job_name == "memberships.entitlements.sync"
                })
        );
        assert!(
            plan.jobs_domain
                .handlers
                .iter()
                .any(|handler| handler.id.as_str() == "memberships.entitlements.sync")
        );
        assert!(plan.module_search_contributions.iter().any(|registered| {
            registered.module == "commerce"
                && registered.contribution.id == "search.catalog.products"
        }));
        assert!(plan.module_report_definitions.iter().any(|registered| {
            registered.module == "memberships"
                && registered.definition.id == "report.memberships.summary"
        }));
        assert!(plan.module_bulk_operations.iter().any(|registered| {
            registered.module == "events" && registered.definition.id == "bulk.events.check-in"
        }));
        assert!(
            plan.ops_catalog
                .reports
                .definition(&ReportId::new("report.memberships.summary").unwrap())
                .is_some()
        );
        assert!(
            plan.ops_catalog
                .bulk
                .definition(&BulkOperationId::new("bulk.events.check-in").unwrap())
                .is_some()
        );
    }

    #[test]
    fn rejects_duplicate_route_names_for_the_same_method() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(RouteDefinition::new("events.show", HttpMethod::Get, "/events").unwrap())
            .with_route(
                RouteDefinition::new("events.show", HttpMethod::Get, "/events-duplicate").unwrap(),
            )
            .build()
            .unwrap_err();

        match error {
            RuntimeBuildError::Route(RouteBuildError::DuplicateRoute { name, method }) => {
                assert_eq!(name, "events.show");
                assert_eq!(method, HttpMethod::Get);
            }
            other => panic!("expected duplicate route error, got {other:?}"),
        }
    }

    #[test]
    fn execute_request_derives_context_and_session_from_cookie() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                    .unwrap()
                    .with_area(RouteArea::Account)
                    .requiring_session(),
            )
            .with_handler(
                HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap(),
            )
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();

        let execution = plan
            .execute_request(
                RequestInput::new(HttpMethod::Get, "www.example.com", "/account")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .with_principal("user-1"),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(execution.customer_app, "showcase-events");
        assert_eq!(execution.route.route_name, "account.dashboard");
        assert_eq!(execution.route_area, RouteArea::Account);
        assert_eq!(execution.locale, "en-GB");
        assert_eq!(execution.session.session_id.as_deref(), Some("session-123"));
        assert!(execution.session.resolved_from_cookie);
        assert_eq!(execution.cache, CacheDisposition::Private);
        assert_eq!(
            execution
                .cache_plan
                .headers
                .get("Cache-Control")
                .map(String::as_str),
            Some("private, max-age=60, stale-while-revalidate=30")
        );
        assert!(
            execution
                .cache_plan
                .headers
                .get("X-Davenda-Variation-Key")
                .is_some()
        );
        assert_eq!(execution.trace.transport_scheme, "https");
        assert_eq!(execution.middleware, plan.http.middleware);
        assert_eq!(
            execution.response,
            HandlerResponse::Page(PageResponse {
                template: "account/dashboard".to_string(),
                status: 200,
            })
        );
    }

    #[test]
    fn execute_request_enforces_csrf_for_state_changing_browser_routes() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("cms.publish", HttpMethod::Post, "/admin/pages/publish")
                    .unwrap()
                    .with_area(RouteArea::Admin)
                    .requiring_session(),
            )
            .with_handler(HandlerDefinition::redirect("cms.publish", "/admin/pages").unwrap())
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();

        let missing_token = plan.execute_request(
            RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
                .unwrap()
                .with_session_cookie(session_cookie.clone()),
            cookie_secret,
            csrf_secret,
        );
        assert_eq!(
            missing_token.unwrap_err(),
            RequestExecutionError::MissingCsrfToken {
                route: "cms.publish".to_string(),
            }
        );

        let token = plan
            .browser
            .csrf
            .issue_token(csrf_secret, "session-123", "cms.publish")
            .unwrap();
        let execution = plan
            .execute_request(
                RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .with_csrf_token(token),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(execution.cache, CacheDisposition::Uncacheable);
        assert_eq!(
            execution
                .cache_plan
                .headers
                .get("Cache-Control")
                .map(String::as_str),
            Some("no-store")
        );
        assert!(execution.cache_plan.plan.application().is_none());
        assert_eq!(execution.route.route_name, "cms.publish");
        assert_eq!(
            execution.response,
            HandlerResponse::Redirect(RedirectResponse {
                location: "/admin/pages".to_string(),
                status: 303,
            })
        );
    }

    #[test]
    fn execute_request_requires_capability_for_capability_gated_routes() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("cms.preview", HttpMethod::Get, "/admin/pages/preview")
                    .unwrap()
                    .with_area(RouteArea::Admin)
                    .requiring_capability(davenda_auth::Capability::CmsPageRead),
            )
            .with_handler(
                HandlerDefinition::fragment("cms.preview", "cms/preview", "preview-pane").unwrap(),
            )
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();

        let denied = plan.execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/pages/preview")
                .unwrap()
                .with_session_cookie(session_cookie.clone()),
            cookie_secret,
            csrf_secret,
        );
        assert_eq!(
            denied.unwrap_err(),
            RequestExecutionError::CapabilityRequired {
                route: "cms.preview".to_string(),
                capability: davenda_auth::Capability::CmsPageRead,
            }
        );

        let allowed = plan
            .execute_request(
                RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/pages/preview")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .grant_capability(davenda_auth::Capability::CmsPageRead),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(allowed.route.route_name, "cms.preview");
        assert_eq!(allowed.cache, CacheDisposition::Private);
        assert_eq!(
            allowed
                .cache_plan
                .headers
                .get("Cache-Control")
                .map(String::as_str),
            Some("private, max-age=30, stale-while-revalidate=15")
        );
        assert_eq!(
            allowed.response,
            HandlerResponse::Fragment(FragmentResponse {
                template: "cms/preview".to_string(),
                fragment_id: "preview-pane".to_string(),
            })
        );
    }

    #[test]
    fn cache_host_stores_and_revalidates_public_route_responses() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("events.public", HttpMethod::Get, "/events")
                    .unwrap()
                    .localized(),
            )
            .with_handler(HandlerDefinition::page("events.public", "events/list").unwrap())
            .build()
            .unwrap();
        let execution = plan
            .execute_request(
                RequestInput::new(HttpMethod::Get, "www.example.com", "/en-GB/events").unwrap(),
                b"01234567012345670123456701234567",
                b"76543210765432107654321076543210",
            )
            .unwrap();

        assert_eq!(execution.cache, CacheDisposition::Public);
        assert_eq!(
            execution
                .cache_plan
                .headers
                .get("Cache-Control")
                .map(String::as_str),
            Some("public, max-age=300, stale-while-revalidate=30")
        );
        assert!(execution.cache_plan.headers.get("Surrogate-Key").is_some());

        let mut host = plan.cache_host().unwrap();
        assert!(
            host.lookup_execution(&execution, CacheInstant::from_unix_seconds(100))
                .is_some_and(|lookup| lookup.state == CacheLookupState::Miss)
        );

        let fill = host
            .begin_fill(&execution, "renderer-1")
            .expect("public route should be application-cacheable");
        assert!(matches!(fill, FillDecision::Start(_)));
        let key = host
            .store_execution(
                &execution,
                "<html>events</html>",
                CacheInstant::from_unix_seconds(100),
            )
            .expect("public route should store into the cache");
        host.complete_fill(&fill).unwrap();

        let fresh = host
            .lookup_execution(&execution, CacheInstant::from_unix_seconds(110))
            .expect("public route should look up through the cache host");
        assert_eq!(fresh.state, CacheLookupState::Fresh);
        assert_eq!(
            fresh.entry.as_ref().map(|entry| entry.key.clone()),
            Some(key.clone())
        );

        let invalidated = host.invalidate(execution.cache_plan.plan.application().unwrap().tags());
        assert_eq!(invalidated, vec![key]);
    }

    #[test]
    fn execute_request_blocks_routes_when_feature_flag_is_disabled() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let feature_flag = FeatureFlag::new("beta-events", false)
            .unwrap()
            .with_rule(
                davenda_observability::FlagTarget::CustomerApp(
                    FlagCustomerAppId::new("other-app").unwrap(),
                ),
                true,
            )
            .unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new("events.beta", HttpMethod::Get, "/events/beta")
                    .unwrap()
                    .with_feature_flag("beta-events"),
            )
            .with_handler(HandlerDefinition::page("events.beta", "events/beta").unwrap())
            .with_feature_flag(feature_flag)
            .build()
            .unwrap();

        let error = plan.execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/events/beta").unwrap(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        );

        assert_eq!(
            error.unwrap_err(),
            RequestExecutionError::FeatureFlagDisabled {
                route: "events.beta".to_string(),
                feature_flag: "beta-events".to_string(),
            }
        );
    }

    #[test]
    fn execute_request_respects_maintenance_mode_with_operator_bypass() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let maintenance = davenda_observability::MaintenanceMode {
            enabled: true,
            audience: MaintenanceAudience::CustomerApp(
                FlagCustomerAppId::new("showcase-events").unwrap(),
            ),
            impact: MaintenanceImpact::MutatingTrafficOnly,
            bypass_token: Some("ops-bypass".to_string()),
            allowed_background_work: BTreeSet::new(),
        };
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_route(
                RouteDefinition::new(
                    "admin.bulk-publish",
                    HttpMethod::Post,
                    "/admin/bulk/publish",
                )
                .unwrap()
                .with_area(RouteArea::Admin)
                .requiring_session(),
            )
            .with_handler(
                HandlerDefinition::json(
                    "admin.bulk-publish",
                    BTreeMap::from([("status".to_string(), "queued".to_string())]),
                )
                .unwrap(),
            )
            .with_maintenance_mode(maintenance)
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-123")
            .unwrap();
        let token = plan
            .browser
            .csrf
            .issue_token(csrf_secret, "session-123", "admin.bulk-publish")
            .unwrap();

        let blocked = plan.execute_request(
            RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/bulk/publish")
                .unwrap()
                .with_session_cookie(session_cookie.clone())
                .with_csrf_token(token.clone()),
            cookie_secret,
            csrf_secret,
        );
        assert_eq!(
            blocked.unwrap_err(),
            RequestExecutionError::MaintenanceMode {
                route: "admin.bulk-publish".to_string(),
            }
        );

        let allowed = plan
            .execute_request(
                RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/bulk/publish")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .with_csrf_token(token)
                    .with_maintenance_bypass_token("ops-bypass"),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();
        assert_eq!(
            allowed.response,
            HandlerResponse::Json(JsonResponse {
                status: 200,
                payload: BTreeMap::from([("status".to_string(), "queued".to_string())]),
            })
        );
    }

    #[test]
    fn runtime_builder_rejects_missing_required_module_dependencies() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(MembershipsModule::new())
            .build()
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeBuildError::ModuleInstallation(ModuleInstallationError::MissingModuleDependency {
                module,
                dependency,
            }) if module == "memberships" && dependency == "commerce"
        ));
    }

    #[test]
    fn runtime_builder_materializes_jobs_domain_for_module_subscriptions() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(MembershipsModule::new())
            .with_module(EventsModule::new())
            .build()
            .unwrap();

        assert!(plan.registered_runtime_jobs.iter().any(|job| {
            job.contract.name == "memberships.entitlements.sync"
                && job.queue == plan.jobs.topology.domain_events_queue
        }));
        assert!(
            plan.registered_runtime_event_subscriptions
                .iter()
                .any(|subscription| {
                    subscription.job_name == "memberships.entitlements.sync"
                        && subscription.event_type.as_str() == "commerce.order.paid"
                })
        );
        assert!(plan.jobs_domain.validate().is_ok());
    }

    #[test]
    fn jobs_host_dispatches_domain_events_and_retries_declared_jobs() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(MembershipsModule::new())
            .build()
            .unwrap();
        let mut host = plan.jobs_host("scheduler-a").unwrap();

        let dispatch = host
            .emit_domain_event(
                DomainEventDispatchRequest::new(
                    "commerce.order.paid",
                    "order",
                    "order-42",
                    "membership checkout completed",
                )
                .unwrap(),
                JobInstant::from_unix_seconds(200),
            )
            .unwrap();

        assert_eq!(dispatch.event_type.as_str(), "commerce.order.paid");
        assert_eq!(dispatch.enqueued_jobs.len(), 2);
        assert_eq!(host.coordinator().ready_jobs().len(), 2);
        assert!(host.coordinator().ready_jobs().iter().any(|record| {
            record.spec.job_name.as_str() == "event-handler:memberships.entitlements.sync"
        }));

        let first_lease = host
            .lease_ready_jobs(
                &plan.jobs.topology.domain_events_queue,
                "worker-a",
                JobInstant::from_unix_seconds(200),
                Duration::from_secs(30),
                1,
            )
            .unwrap()
            .remove(0);
        let retry = host
            .acknowledge_failed(
                &first_lease,
                JobInstant::from_unix_seconds(205),
                DeadLetterReason::PolicyViolation,
                "temporary membership projection failure",
            )
            .unwrap();

        assert!(matches!(
            retry,
            JobFailureDisposition::Retried { ref queue, .. }
                if queue == &plan.jobs.topology.domain_events_queue
        ));
    }

    #[test]
    fn jobs_host_requires_scheduler_leadership_for_scheduled_jobs() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(CmsModule::new())
            .build()
            .unwrap();
        let mut host = plan.jobs_host("scheduler-a").unwrap();
        let scheduled =
            JobDispatchRequest::new("cms.publish-scheduled", "publish scheduled landing page")
                .unwrap()
                .scheduled_for(JobInstant::from_unix_seconds(120))
                .with_idempotency_key("cms.publish-scheduled:page-42")
                .unwrap();

        let job_id = host
            .enqueue_job(scheduled, JobInstant::from_unix_seconds(100))
            .unwrap();
        assert_eq!(host.coordinator().scheduled_jobs().len(), 1);

        let err = host
            .promote_due_jobs(JobInstant::from_unix_seconds(120))
            .unwrap_err();
        assert!(matches!(
            err,
            RuntimeJobsError::Jobs(JobsModelError::MissingSchedulerLeadership { node_id })
                if node_id == "scheduler-a"
        ));

        host.acquire_scheduler_leadership(
            JobInstant::from_unix_seconds(110),
            Duration::from_secs(60),
        )
        .unwrap();
        let promoted = host
            .promote_due_jobs(JobInstant::from_unix_seconds(120))
            .unwrap();
        assert_eq!(promoted, vec![job_id]);
        assert_eq!(host.coordinator().ready_jobs().len(), 1);
    }

    #[test]
    fn jobs_host_rejects_domain_event_jobs_without_event_dispatch() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(MembershipsModule::new())
            .build()
            .unwrap();
        let mut host = plan.jobs_host("scheduler-a").unwrap();
        let request = JobDispatchRequest::new(
            "memberships.entitlements.sync",
            "attempt to bypass event flow",
        )
        .unwrap()
        .with_idempotency_key("memberships.entitlements.sync:42")
        .unwrap();

        let err = host
            .enqueue_job(request, JobInstant::from_unix_seconds(50))
            .unwrap_err();
        assert_eq!(
            err,
            RuntimeJobsError::DomainEventJobRequiresEventDispatch {
                job: "memberships.entitlements.sync".to_string(),
            }
        );
    }

    #[test]
    fn runtime_builder_rejects_duplicate_runtime_job_names() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let first = StaticManifestModule::new(ModuleManifest::new("invalid").with_jobs(vec![
            JobContract::new(
                "shared.job",
                JobTriggerKind::Operator,
                true,
                "first copy of a duplicated runtime job",
            ),
        ]));
        let second = StaticManifestModule::new(ModuleManifest::new("other").with_jobs(vec![
            JobContract::new(
                "shared.job",
                JobTriggerKind::Webhook,
                true,
                "second copy of a duplicated runtime job",
            ),
        ]));

        let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(first)
            .with_module(second)
            .build()
            .unwrap_err();

        match error {
            RuntimeBuildError::DuplicateRuntimeJobName {
                job,
                first_module,
                second_module,
            } => {
                assert_eq!(job, "shared.job");
                assert_eq!(first_module, "invalid");
                assert_eq!(second_module, "other");
            }
            other => panic!("expected duplicate runtime job error, got {other:?}"),
        }
    }

    #[test]
    fn ops_host_queues_report_exports_into_the_jobs_runtime() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(AdminModule::new())
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(MembershipsModule::new())
            .with_module(OpsModule::new())
            .build()
            .unwrap();
        let mut ops = plan.ops_host("scheduler-a").unwrap();

        let queued = ops
            .queue_report_export(
                ReportExportRequest::new(
                    ReportExportId::new("export-memberships-1").unwrap(),
                    ReportId::new("report.memberships.summary").unwrap(),
                    "operator-1",
                    JobInstant::from_unix_seconds(100),
                )
                .unwrap()
                .with_capability(Capability::MembershipSubscriptionManage)
                .with_idempotency_key(IdempotencyKey::new("report:memberships:summary:1").unwrap()),
            )
            .unwrap();

        assert_eq!(
            queued.plan.definition.id.as_str(),
            "report.memberships.summary"
        );
        assert_eq!(queued.queued_job_id.as_str(), "export-memberships-1");
        assert_eq!(ops.jobs().coordinator().ready_jobs().len(), 1);
        assert_eq!(
            ops.jobs().coordinator().ready_jobs()[0]
                .spec
                .job_name
                .as_str(),
            "report.export.report.memberships.summary"
        );
    }

    #[test]
    fn ops_host_enforces_bulk_capabilities_before_queueing_jobs() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(AdminModule::new())
            .with_module(CmsModule::new())
            .with_module(CommerceModule::new())
            .with_module(EventsModule::new())
            .with_module(OpsModule::new())
            .build()
            .unwrap();
        let mut ops = plan.ops_host("scheduler-a").unwrap();

        let denied = ops
            .queue_bulk_operation(
                BulkOperationRequest::new(
                    BulkExecutionId::new("bulk-check-in-1").unwrap(),
                    BulkOperationId::new("bulk.events.check-in").unwrap(),
                    "operator-1",
                    JobInstant::from_unix_seconds(100),
                    25,
                )
                .unwrap()
                .with_idempotency_key(IdempotencyKey::new("bulk:events:check-in:1").unwrap()),
            )
            .unwrap_err();
        assert!(matches!(
            denied,
            RuntimeOpsError::Ops(OpsModelError::MissingCapability {
                operation: "bulk operation",
                required: Capability::EventsBookingCheckIn,
            })
        ));

        let queued = ops
            .queue_bulk_operation(
                BulkOperationRequest::new(
                    BulkExecutionId::new("bulk-check-in-2").unwrap(),
                    BulkOperationId::new("bulk.events.check-in").unwrap(),
                    "operator-1",
                    JobInstant::from_unix_seconds(110),
                    25,
                )
                .unwrap()
                .with_capability(Capability::EventsBookingCheckIn)
                .with_idempotency_key(IdempotencyKey::new("bulk:events:check-in:2").unwrap()),
            )
            .unwrap();

        assert_eq!(queued.queued_job_id.as_str(), "bulk-check-in-2");
        assert_eq!(ops.jobs().coordinator().ready_jobs().len(), 1);
        assert_eq!(
            ops.jobs().coordinator().ready_jobs()[0]
                .spec
                .job_name
                .as_str(),
            "bulk.bulk.events.check-in"
        );
    }

    #[test]
    fn runtime_plan_creates_tls_host_with_expected_provider_mode() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let host = plan.tls_host();
        let status = host.status();

        assert_eq!(status.customer_app, "showcase-events");
        assert_eq!(status.mode, davenda_config::TlsMode::Acme);
        assert_eq!(status.edge_mode, EdgeMode::DirectTermination);
        assert_eq!(
            status.provider,
            Some(CertificateProviderKind::CloudflareDns)
        );
        assert!(status.inventory.certificates().is_empty());
    }

    #[test]
    fn tls_host_status_tracks_inventory_renewals_and_pending_challenges() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let mut host = plan.tls_host();
        let certificate_id = CertificateId::new("cert-live").unwrap();

        host.import_certificate(active_certificate("cert-live", "www.example.com"))
            .unwrap();
        host.queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
            .unwrap();
        host.begin_renewal(&certificate_id, CertificateId::new("cert-next").unwrap())
            .unwrap();

        let status = host.status();
        assert_eq!(status.inventory.certificates().len(), 1);
        assert_eq!(status.queued_renewals.len(), 1);
        assert_eq!(status.pending_challenges.len(), 1);
        assert_eq!(
            status.pending_challenges[0]
                .replacement_certificate_id
                .as_ref()
                .map(|id| id.as_str()),
            Some("cert-next")
        );
    }

    #[test]
    fn tls_host_activate_replacement_emits_hot_reload_and_supersedes_old_certificate() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let mut host = plan.tls_host();
        let certificate_id = CertificateId::new("cert-live").unwrap();

        host.import_certificate(active_certificate("cert-live", "shop.example.com"))
            .unwrap();
        host.queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
            .unwrap();
        host.begin_renewal(&certificate_id, CertificateId::new("cert-next").unwrap())
            .unwrap();

        let event = host
            .activate_replacement(
                &certificate_id,
                active_certificate("cert-next", "shop.example.com")
                    .with_cloudflare_mode(CloudflareEncryptionMode::FullStrict),
            )
            .unwrap();
        assert_eq!(event.certificate_id.as_str(), "cert-next");
        assert!(event.reloaded_without_restart);

        let status = host.status();
        assert_eq!(status.hot_reload_events.len(), 1);
        assert_eq!(
            status
                .inventory
                .active_for_hostname(&Hostname::new("shop.example.com").unwrap())
                .unwrap()
                .id
                .as_str(),
            "cert-next"
        );
        assert_eq!(
            status.inventory.record(&certificate_id).unwrap().status,
            CertificateStatus::Superseded
        );
    }

    #[test]
    fn tls_host_rejects_external_termination_issuance_and_preserves_cloudflare_origin_mode() {
        let external = PlatformConfig::from_toml_str(&external_tls_config()).unwrap();
        let external_plan = RuntimeBuilder::new(external, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let external_host = external_plan.tls_host();

        let err = external_host
            .issue_for_bindings(vec![HostnameBinding::new(
                Hostname::new("www.example.com").unwrap(),
                CustomerAppId::new("showcase-events").unwrap(),
            )])
            .unwrap_err();
        assert_eq!(
            err,
            RuntimeTlsError::Tls(TlsModelError::ExternalTerminationDoesNotIssue)
        );

        let origin = PlatformConfig::from_toml_str(&cloudflare_origin_tls_config()).unwrap();
        let origin_plan = RuntimeBuilder::new(origin, DefaultAuthModelPackage::default())
            .build()
            .unwrap();
        let issue_plan = origin_plan
            .tls_host()
            .issue_for_bindings(vec![HostnameBinding::new(
                Hostname::new("origin.example.com").unwrap(),
                CustomerAppId::new("showcase-events").unwrap(),
            )])
            .unwrap();

        assert_eq!(
            issue_plan.cloudflare_mode,
            Some(CloudflareEncryptionMode::FullStrict)
        );
    }

    #[test]
    fn runtime_builder_registers_installed_extensions_against_declared_slots() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(AdminModule::new())
            .with_installed_extension(installed_admin_widget_extension())
            .build()
            .unwrap();

        assert_eq!(plan.installed_extensions.len(), 1);
        assert_eq!(plan.installed_extensions[0].extension_id, "admin.waitlist");
        assert!(plan.registered_extension_slots.iter().any(|slot| {
            slot.module == "admin"
                && slot.kind == ExtensionPointKind::AdminWidget
                && slot.surface == "admin.dashboard.summary"
        }));
        assert_eq!(
            plan.extension_registry
                .admin_widget_handlers("admin.dashboard.summary")
                .len(),
            1
        );
    }

    #[test]
    fn runtime_builder_rejects_extensions_without_declared_slots() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_installed_extension(installed_admin_widget_extension())
            .build()
            .unwrap_err();

        match error {
            RuntimeBuildError::UnknownExtensionSlot {
                extension_id,
                handler_id,
                point,
                surface,
            } => {
                assert_eq!(extension_id, "admin.waitlist");
                assert_eq!(handler_id, "waitlist-summary");
                assert_eq!(point, ExtensionPointKind::AdminWidget);
                assert_eq!(surface, "admin.dashboard.summary");
            }
            other => panic!("expected unknown extension slot, got {other:?}"),
        }
    }

    #[test]
    fn runtime_builder_materializes_module_http_surfaces() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(MediaModule::new())
            .build()
            .unwrap();

        let cookie_secret = b"01234567012345670123456701234567";
        let csrf_secret = b"76543210765432107654321076543210";
        let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
            .sign(cookie_secret, "session-456")
            .unwrap();

        let execution = plan
            .execute_request(
                RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/media")
                    .unwrap()
                    .with_session_cookie(session_cookie)
                    .grant_capability(davenda_auth::Capability::AssetRead),
                cookie_secret,
                csrf_secret,
            )
            .unwrap();

        assert_eq!(execution.route.route_name, "media.library");
        assert_eq!(execution.route_area, RouteArea::Admin);
        assert_eq!(
            execution.response,
            HandlerResponse::Page(PageResponse {
                template: "media/library".to_string(),
                status: 200,
            })
        );
    }

    #[test]
    fn runtime_builder_matches_parameterized_module_routes() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(EventsModule::new())
            .build()
            .unwrap();

        let execution = plan
            .execute_request(
                RequestInput::new(
                    HttpMethod::Get,
                    "www.example.com",
                    "/en-GB/events/summer-gala",
                )
                .unwrap(),
                b"01234567012345670123456701234567",
                b"76543210765432107654321076543210",
            )
            .unwrap();

        assert_eq!(execution.route.route_name, "events.detail");
        assert_eq!(execution.locale, "en-GB");
        assert_eq!(
            execution.route.params.get("event_slug").map(String::as_str),
            Some("summer-gala")
        );
        assert_eq!(
            execution.response,
            HandlerResponse::Page(PageResponse {
                template: "events/detail".to_string(),
                status: 200,
            })
        );
    }

    #[test]
    fn http_runtime_generates_named_paths_for_module_routes() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(EventsModule::new())
            .build()
            .unwrap();
        let params = BTreeMap::from([("event_slug".to_string(), "summer-gala".to_string())]);

        let path = plan
            .http
            .path_for(&plan.config, "events.detail", &params, Some("fr-FR"))
            .unwrap();
        assert_eq!(path, "/fr-FR/events/summer-gala");

        let absolute = plan
            .http
            .absolute_url_for(&plan.config, "events.detail", &params, Some("en-GB"))
            .unwrap();
        assert_eq!(absolute, "https://www.example.com/en-GB/events/summer-gala");

        let missing = plan
            .http
            .path_for(
                &plan.config,
                "events.detail",
                &BTreeMap::new(),
                Some("en-GB"),
            )
            .unwrap_err();
        assert_eq!(
            missing,
            RouteUrlError::MissingRouteParameter {
                route: "events.detail".to_string(),
                parameter: "event_slug".to_string(),
            }
        );
    }
}

fn build_http_runtime_plan<P>(
    package: &P,
    routes: &[RouteDefinition],
) -> Result<HttpRuntimePlan, RouteBuildError>
where
    P: AuthModelPackage,
{
    let mut seen = std::collections::BTreeSet::new();
    for route in routes {
        if !seen.insert((route.name.clone(), route.method)) {
            return Err(RouteBuildError::DuplicateRoute {
                name: route.name.clone(),
                method: route.method,
            });
        }

        if let RouteAuthGate::Capability(capability) = route.auth {
            if package.binding_for(capability).is_none() {
                return Err(RouteBuildError::MissingCapabilityBinding {
                    name: route.name.clone(),
                    capability,
                });
            }
        }
    }

    Ok(HttpRuntimePlan {
        middleware: vec![
            MiddlewareStage::TransportNormalization,
            MiddlewareStage::CustomerAppResolution,
            MiddlewareStage::TraceContext,
            MiddlewareStage::LocaleResolution,
            MiddlewareStage::SessionResolution,
            MiddlewareStage::BrowserPolicy,
            MiddlewareStage::ResponsePolicy,
        ],
        routes: routes.to_vec(),
    })
}

fn validate_route_name(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyRouteName)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_route_path(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.starts_with('/') && !trimmed.is_empty() {
        Ok(trimmed.to_string())
    } else {
        Err(RouteBuildError::InvalidRoutePath {
            path: value.trim().to_string(),
        })
    }
}

fn validate_host(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyHostPattern)
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_template_name(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}

fn validate_fragment_id(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}
