use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataRepositoryPrincipalBinding {
    Omit,
    InvocationPrincipal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRepositoryQueryProfile {
    pub page: PageRequest,
    pub publication_visibility: PublicationVisibility,
    pub default_cache_scope: QueryCacheScope,
    pub localized_cache_scope: QueryCacheScope,
    pub principal_binding: DataRepositoryPrincipalBinding,
}

impl DataRepositoryQueryProfile {
    pub fn new(
        page: PageRequest,
        publication_visibility: PublicationVisibility,
        cache_scope: QueryCacheScope,
    ) -> Self {
        Self {
            page,
            publication_visibility,
            default_cache_scope: cache_scope,
            localized_cache_scope: cache_scope,
            principal_binding: DataRepositoryPrincipalBinding::Omit,
        }
    }

    pub fn with_localized_cache_scope(mut self, cache_scope: QueryCacheScope) -> Self {
        self.localized_cache_scope = cache_scope;
        self
    }

    pub fn bind_invocation_principal(mut self) -> Self {
        self.principal_binding = DataRepositoryPrincipalBinding::InvocationPrincipal;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRepositoryContribution {
    pub id: String,
    pub repository: RepositorySpec,
    pub query_profile: DataRepositoryQueryProfile,
}

impl DataRepositoryContribution {
    pub fn new(repository: RepositorySpec, query_profile: DataRepositoryQueryProfile) -> Self {
        Self {
            id: repository.id.clone(),
            repository,
            query_profile,
        }
    }
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
    pub data_repositories: Vec<DataRepositoryContribution>,
    pub search_contributions: Vec<SearchIndexContribution>,
    pub report_definitions: Vec<ReportDefinition>,
    pub bulk_operations: Vec<BulkOperationDefinition>,
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
            data_repositories: Vec::new(),
            search_contributions: Vec::new(),
            report_definitions: Vec::new(),
            bulk_operations: Vec::new(),
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

    pub fn with_event_subscriptions(mut self, subscriptions: Vec<EventSubscription>) -> Self {
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

    pub fn with_data_repositories(
        mut self,
        data_repositories: Vec<DataRepositoryContribution>,
    ) -> Self {
        self.data_repositories = data_repositories;
        self
    }

    pub fn with_search_contributions(
        mut self,
        search_contributions: Vec<SearchIndexContribution>,
    ) -> Self {
        self.search_contributions = search_contributions;
        self
    }

    pub fn with_report_definitions(mut self, report_definitions: Vec<ReportDefinition>) -> Self {
        self.report_definitions = report_definitions;
        self
    }

    pub fn with_bulk_operations(mut self, bulk_operations: Vec<BulkOperationDefinition>) -> Self {
        self.bulk_operations = bulk_operations;
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
    pub fn required(module: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            kind: ModuleDependencyKind::Required,
            reason: reason.into(),
        }
    }

    pub fn optional(module: impl Into<String>, reason: impl Into<String>) -> Self {
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
            Self::BrowserSecurity => &["core.http.sessions", "core.http.cookies", "core.http.csrf"],
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
    pub fn new(owner: impl Into<String>, order: u32, description: impl Into<String>) -> Self {
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
    pub fn new(name: impl Into<String>, kind: RouteSurfaceKind, path: impl Into<String>) -> Self {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDocumentKind {
    Page,
    Product,
    Collection,
    Event,
    EventSlot,
    Booking,
    Media,
    MembershipSubscription,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFieldRole {
    Title,
    Summary,
    Body,
    Keyword,
    Facet,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchVisibility {
    Public,
    Authenticated,
    Capability(Capability),
}

impl SearchVisibility {
    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        match self {
            Self::Public => true,
            Self::Authenticated => !capabilities.is_empty(),
            Self::Capability(capability) => capabilities.contains(capability),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchInvalidationTrigger {
    Published,
    Updated,
    Unpublished,
    Deleted,
    ManualRebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchRebuildStrategy {
    OnInvalidate,
    Scheduled { interval: Duration },
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFieldContribution {
    pub id: String,
    pub source_path: String,
    pub role: SearchFieldRole,
    pub stored: bool,
    pub searchable: bool,
}

impl SearchFieldContribution {
    pub fn new(
        id: impl Into<String>,
        source_path: impl Into<String>,
        role: SearchFieldRole,
        stored: bool,
        searchable: bool,
    ) -> Self {
        Self {
            id: id.into(),
            source_path: source_path.into(),
            role,
            stored,
            searchable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInvalidationRule {
    pub trigger: SearchInvalidationTrigger,
    pub reason: String,
}

impl SearchInvalidationRule {
    pub fn new(trigger: SearchInvalidationTrigger, reason: impl Into<String>) -> Self {
        Self {
            trigger,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexContribution {
    pub id: String,
    pub document_kind: SearchDocumentKind,
    pub visibility: SearchVisibility,
    pub publication_required: bool,
    pub fields: Vec<SearchFieldContribution>,
    pub invalidation_rules: Vec<SearchInvalidationRule>,
    pub rebuild_strategy: SearchRebuildStrategy,
}

impl SearchIndexContribution {
    pub fn new(
        id: impl Into<String>,
        document_kind: SearchDocumentKind,
        visibility: SearchVisibility,
        publication_required: bool,
        fields: Vec<SearchFieldContribution>,
        invalidation_rules: Vec<SearchInvalidationRule>,
        rebuild_strategy: SearchRebuildStrategy,
    ) -> Self {
        Self {
            id: id.into(),
            document_kind,
            visibility,
            publication_required,
            fields,
            invalidation_rules,
            rebuild_strategy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Csv,
    Json,
    Pdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSensitivity {
    Public,
    Internal,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportDeliveryMode {
    PublicObjectStore,
    SignedUrl,
    InternalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDefinition {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub format: ReportFormat,
    pub sensitivity: ReportSensitivity,
    pub delivery_mode: ReportDeliveryMode,
    pub export_prefix: String,
    pub retry_policy: RetryPolicy,
}

impl ReportDefinition {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        format: ReportFormat,
        sensitivity: ReportSensitivity,
        delivery_mode: ReportDeliveryMode,
        export_prefix: impl Into<String>,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description,
            required_capability,
            format,
            sensitivity,
            delivery_mode,
            export_prefix: export_prefix.into(),
            retry_policy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationKind {
    Publish,
    Unpublish,
    Reindex,
    Export,
    Cancel,
    CheckIn,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationScope {
    Cms,
    Commerce,
    Memberships,
    Events,
    Media,
    Search,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationDefinition {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub kind: BulkOperationKind,
    pub scope: BulkOperationScope,
    pub retry_policy: RetryPolicy,
    pub max_items: Option<usize>,
    pub requires_idempotency_key: bool,
}

impl BulkOperationDefinition {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        kind: BulkOperationKind,
        scope: BulkOperationScope,
        retry_policy: RetryPolicy,
        max_items: Option<usize>,
        requires_idempotency_key: bool,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description,
            required_capability,
            kind,
            scope,
            retry_policy,
            max_items,
            requires_idempotency_key,
        }
    }
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

