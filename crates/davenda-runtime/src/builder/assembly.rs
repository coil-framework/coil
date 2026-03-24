use super::*;
use crate::builder::helpers::*;
use crate::builder::http::*;
use crate::builder::templates;
use crate::builder::state::RuntimeBuilderParts;
use crate::plan::shared_state_root;
use davenda_template::TemplateRuntime;

pub(crate) fn build_runtime_plan<P>(
    builder: RuntimeBuilder<P>,
) -> Result<RuntimePlan, RuntimeBuildError>
where
    P: AuthModelPackage + 'static,
{
    let RuntimeBuilderParts {
        config,
        auth_package,
        modules,
        extensions,
        templates,
        template_roots,
        storage_policies,
        routes,
        handlers,
        feature_flags,
        maintenance_mode,
    } = builder.into_parts();

    config.validate().map_err(ConfigError::Validation)?;

    if auth_package.manifest().name != config.auth.package {
        return Err(RuntimeBuildError::AuthPackageMismatch {
            configured: config.auth.package,
            actual: auth_package.manifest().name.clone(),
        });
    }

    let bootstrap = bootstrap_core_services(&config)?;
    let storage_planner =
        StoragePlanner::new(StorageTopology::from_config(&config), storage_policies);
    let mut registry = bootstrap.registry;
    let mut template = bootstrap.template;
    let mut observability = bootstrap.observability;
    let mut module_manifests = Vec::new();
    let mut install_migrations = MigrationPlan::new();

    for feature_flag in feature_flags {
        observability.flags.insert(feature_flag)?;
    }

    if let Some(maintenance_mode) = maintenance_mode {
        observability.maintenance = maintenance_mode;
    }

    let mut installed_modules = Vec::new();
    let mut collected_modules = Vec::new();

    for module in modules {
        let manifest = module.manifest();
        validate_module_capabilities(&auth_package, &manifest)?;
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
    let mut all_routes = routes;
    all_routes.extend(module_routes);
    let mut all_handlers = handlers;
    all_handlers.extend(module_handlers);
    let http = build_http_runtime_plan(&auth_package, &all_routes)?;
    let handlers = build_handler_registry(&all_routes, all_handlers)?;

    for (module, _) in collected_modules {
        if let Some(plan) = module.install_migration_plan() {
            for step in plan.ordered_steps().iter().cloned() {
                install_migrations.insert(step)?;
            }
        }
        module.register(&mut registry)?;
    }

    for definition in templates {
        template.registry.register(definition)?;
    }
    for definition in templates::load_customer_templates_from_roots(
        &template_roots,
        template.customer_app_namespace.clone(),
    )? {
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
    let module_bulk_operations = module_manifests
        .iter()
        .flat_map(|manifest| {
            manifest
                .bulk_operations
                .iter()
                .cloned()
                .map(|definition| RegisteredBulkOperation {
                    module: manifest.name.clone(),
                    definition,
                })
        })
        .collect::<Vec<_>>();
    let ops_catalog = OpsCatalog::from_manifests(&module_manifests)?;
    let registered_extension_slots = collect_extension_slots(&module_manifests)?;
    let mut extension_registry = ExtensionRegistry::new(ContractVersion::new(1, 0, 0));
    let mut installed_extensions = Vec::new();

    for extension in extensions {
        if extension.customer_app_id() != config.app.name {
            return Err(RuntimeBuildError::ExtensionCustomerAppMismatch {
                extension_id: extension.manifest().id.to_string(),
                configured: config.app.name.clone(),
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

    let auth_package = AuthModelPackageSelection::new(auth_package);
    let mut approved_outbound_http_endpoints = BTreeMap::new();
    for integration in &config.wasm.outbound_http {
        approved_outbound_http_endpoints.insert(
            integration.integration.clone(),
            integration.endpoint.clone(),
        );
    }

    let shared_backend_scope = next_runtime_plan_scope();
    let shared_state_root = shared_state_root(&config);

    let app_name = config.app.name.clone();

    Ok(RuntimePlan {
        config,
        auth_package_name: auth_package.manifest().name.clone(),
        auth_package,
        approved_outbound_http_endpoints,
        shared_backend_scope: shared_backend_scope.clone(),
        shared_state_root,
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
        shared_jobs_runtime: SharedJobsRuntimeHandle::new(format!(
            "customer-app:{}:{}",
            app_name, shared_backend_scope
        )),
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
