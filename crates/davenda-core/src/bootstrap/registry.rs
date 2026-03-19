use super::*;

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
