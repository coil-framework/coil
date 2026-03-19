use super::*;

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
