use super::*;
use crate::builder::LinkedCustomerPluginSummary;
use crate::builder::customer_plugins::{CustomerHookSet, RuntimeCustomerHookRegistry};
use crate::builder::helpers::*;
use crate::builder::http::*;
use crate::builder::state::RuntimeBuilderParts;
use crate::builder::templates;
use crate::plan::shared_state_root;
use coil_core::bootstrap_core_services_with_translation_catalogs;
use coil_template::TemplateRuntime;

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
        customer_plugins,
        translation_catalogs,
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

    let bootstrap = bootstrap_core_services_with_translation_catalogs(&config, translation_catalogs)?;
    let mut registry = bootstrap.registry;
    let mut template = bootstrap.template;
    let mut observability = bootstrap.observability;
    let mut module_manifests = Vec::new();
    let mut install_migrations = MigrationPlan::new();
    let mut linked_customer_plugins = Vec::new();
    let mut customer_hooks = CustomerHookSet::default();

    let runtime_routes = routes;
    let runtime_handlers = handlers;
    let runtime_templates = templates;
    let runtime_template_roots = template_roots;
    let runtime_storage_policies = storage_policies;
    let runtime_feature_flags = feature_flags;
    let runtime_maintenance_mode = maintenance_mode;

    let mut seen_customer_plugins = BTreeSet::new();
    for plugin in customer_plugins {
        let descriptor = plugin.descriptor();
        let plugin_id = descriptor.id.trim().to_string();
        if plugin_id.is_empty() {
            return Err(RuntimeBuildError::CustomerPluginRegistration {
                plugin_id: "<empty>".to_string(),
                message: "invalid_input:plugin.id: customer plugin id must not be empty"
                    .to_string(),
            });
        }
        if !seen_customer_plugins.insert(plugin_id.clone()) {
            return Err(RuntimeBuildError::DuplicateCustomerPlugin { plugin_id });
        }

        let mut plugin_registry = RuntimeCustomerHookRegistry::default();
        plugin.register(&mut plugin_registry).map_err(|error| {
            RuntimeBuildError::CustomerPluginRegistration {
                plugin_id: plugin_id.clone(),
                message: error.to_string(),
            }
        })?;
        let (registered_hooks, hook_kinds) = plugin_registry.into_parts();
        customer_hooks.checkout.extend(registered_hooks.checkout);
        customer_hooks.cms.extend(registered_hooks.cms);
        customer_hooks
            .verified_webhooks
            .extend(registered_hooks.verified_webhooks);
        customer_hooks
            .verified_webhook_assets
            .extend(registered_hooks.verified_webhook_assets);
        linked_customer_plugins.push(LinkedCustomerPluginSummary {
            plugin_id,
            display_name: descriptor.display_name,
            version: descriptor.version,
            registered_hooks: hook_kinds,
        });
    }

    let storage_planner = StoragePlanner::new(
        StorageTopology::from_config(&config),
        runtime_storage_policies,
    );

    for feature_flag in runtime_feature_flags {
        observability.flags.insert(feature_flag)?;
    }

    if let Some(maintenance_mode) = runtime_maintenance_mode {
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

    let core_service_id_storage: Vec<String> = registry
        .services()
        .map(|service| service.id.clone())
        .collect();
    let core_service_ids: Vec<&str> = core_service_id_storage
        .iter()
        .map(|service_id| service_id.as_str())
        .collect();

    for (_, manifest) in &collected_modules {
        validate_module_installation(manifest, &installed_modules, &core_service_ids)?;
        registry.register_module_manifest(manifest.clone())?;
        module_manifests.push(manifest.clone());
    }

    let mut customer_templates = templates::load_customer_templates_from_roots(
        &runtime_template_roots,
        template.customer_app_namespace.clone(),
    )?;
    templates::supplement_customer_templates(
        &mut customer_templates,
        template.customer_app_namespace.clone(),
        &module_manifests
            .iter()
            .map(|manifest| manifest.name.clone())
            .collect::<Vec<_>>(),
    )?;

    let (module_routes, module_handlers) = module_http_contributions(&module_manifests)?;
    let mut all_routes = runtime_routes;
    all_routes.extend(module_routes);
    let mut all_handlers = runtime_handlers;
    all_handlers.extend(module_handlers);
    append_customer_home_route(&customer_templates, &mut all_routes, &mut all_handlers)?;
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

    for definition in runtime_templates {
        template.registry.register(definition)?;
    }
    for definition in customer_templates {
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
    let storefront_catalog = StorefrontCatalog::load_from_roots(&runtime_template_roots)?;

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
        storefront_catalog,
        theme_asset_manifest: None,
        template,
        tls: bootstrap.tls,
        wasm: bootstrap.wasm,
        services: registry.services().cloned().collect(),
        modules: module_manifests,
        install_migrations,
        extension_registry,
        registered_extension_slots,
        installed_extensions,
        linked_customer_plugins,
        customer_hooks,
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

fn append_customer_home_route(
    customer_templates: &[coil_template::TemplateDefinition],
    routes: &mut Vec<RouteDefinition>,
    handlers: &mut Vec<HandlerDefinition>,
) -> Result<(), RuntimeBuildError> {
    let has_customer_home_template = customer_templates.iter().any(|template| {
        template.kind == coil_template::TemplateKind::Layout
            && template.key.name.as_str() == "pages/home"
    });
    if !has_customer_home_template {
        return Ok(());
    }

    let has_explicit_root_route = routes
        .iter()
        .any(|route| route.method == HttpMethod::Get && route.path == "/");
    let has_explicit_home_route = routes.iter().any(|route| route.name == "home");
    let has_explicit_home_handler = handlers.iter().any(|handler| handler.route_name == "home");
    if has_explicit_root_route || has_explicit_home_route || has_explicit_home_handler {
        return Ok(());
    }

    routes.push(RouteDefinition::new("home", HttpMethod::Get, "/")?.localized());
    handlers.push(HandlerDefinition::page("home", "pages/home")?);
    Ok(())
}
