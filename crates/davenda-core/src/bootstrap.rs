use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
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
            session_cookie: CookiePolicy::from_config(&config.http.session_cookie),
            flash_cookie: CookiePolicy::from_config(&config.http.flash_cookie),
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
