use super::*;
use davenda_template::{TemplateDefinition, TemplateModelError, TemplateRuntime};

pub struct RuntimeBuilder<P> {
    config: PlatformConfig,
    auth_package: P,
    modules: Vec<Box<dyn PlatformModule>>,
    extensions: Vec<InstalledExtension>,
    templates: Vec<TemplateDefinition>,
    storage_policies: StoragePolicySet,
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
            templates: Vec::new(),
            storage_policies: StoragePolicySet::default(),
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

    pub fn with_template(mut self, template: TemplateDefinition) -> Self {
        self.templates.push(template);
        self
    }

    pub fn with_templates<I>(mut self, templates: I) -> Self
    where
        I: IntoIterator<Item = TemplateDefinition>,
    {
        self.templates.extend(templates);
        self
    }

    pub fn with_storage_policy_rule(mut self, rule: PathPolicyRule) -> Self {
        self.storage_policies = self.storage_policies.with_rule(rule);
        self
    }

    pub fn with_storage_policies(mut self, policies: StoragePolicySet) -> Self {
        self.storage_policies = policies;
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
        let storage_planner = StoragePlanner::new(
            StorageTopology::from_config(&self.config),
            self.storage_policies.clone(),
        );
        let mut registry = bootstrap.registry;
        let mut template = bootstrap.template;
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

        for definition in self.templates {
            template.registry.register(definition)?;
        }
        template.runtime = TemplateRuntime::new(template.registry.clone());

        let mut module_jobs = module_manifests
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
        let module_data_repositories = collect_data_repositories(&module_manifests)?;
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

        module_jobs.extend(collect_extension_runtime_jobs(&extension_registry)?);
        let (registered_runtime_jobs, registered_runtime_event_subscriptions, jobs_domain) =
            build_runtime_jobs_domain(&bootstrap.jobs, &module_jobs, &module_event_subscriptions)?;

        Ok(RuntimePlan {
            config: self.config,
            auth_package_name: self.auth_package.manifest().name.clone(),
            cache_topology: bootstrap.cache.topology,
            cache_planner: bootstrap.cache.planner,
            i18n: bootstrap.i18n,
            seo: bootstrap.seo,
            browser: bootstrap.browser,
            cli: bootstrap.cli,
            data: bootstrap.data,
            jobs: bootstrap.jobs,
            observability,
            http,
            handlers,
            storage_planner,
            template,
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
            module_data_repositories,
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
    #[error(transparent)]
    Template(#[from] TemplateModelError),
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
    #[error(
        "runtime data repository `{repository}` is declared by both `{first_module}` and `{second_module}`"
    )]
    DuplicateDataRepository {
        repository: String,
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

fn collect_data_repositories(
    manifests: &[ModuleManifest],
) -> Result<Vec<RegisteredDataRepository>, RuntimeBuildError> {
    let mut repositories = Vec::new();
    let mut seen = BTreeMap::<String, String>::new();

    for manifest in manifests {
        for contribution in &manifest.data_repositories {
            if let Some(existing_module) =
                seen.insert(contribution.id.clone(), manifest.name.clone())
            {
                return Err(RuntimeBuildError::DuplicateDataRepository {
                    repository: contribution.id.clone(),
                    first_module: existing_module,
                    second_module: manifest.name.clone(),
                });
            }

            repositories.push(RegisteredDataRepository {
                module: manifest.name.clone(),
                contribution: contribution.clone(),
            });
        }
    }

    Ok(repositories)
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
