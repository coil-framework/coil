use super::*;

pub(crate) fn cache_topology_from_config(config: &PlatformConfig) -> CacheTopology {
    match config.cache.l2 {
        Some(DistributedCache::Redis) => CacheTopology::with_redis(),
        Some(DistributedCache::Valkey) => CacheTopology::with_valkey(),
        None => CacheTopology::moka_only(),
    }
}

pub(crate) fn browser_security_from_config(config: &PlatformConfig) -> BrowserSecurityServices {
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

pub(crate) fn observability_runtime_from_config(
    config: &PlatformConfig,
) -> ObservabilityRuntimeServices {
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

pub(crate) fn jobs_runtime_from_config(config: &PlatformConfig) -> JobsRuntimeServices {
    JobsRuntime::from_config(&config.jobs).expect("jobs runtime config must be valid")
}

pub(crate) fn data_runtime_from_config(config: &PlatformConfig) -> DataRuntimeServices {
    DataRuntime::from_config(&config.database).expect("data runtime config must be valid")
}

pub(crate) fn cli_runtime_from_config(config: &PlatformConfig) -> CliRuntimeServices {
    CliRuntimeServices::new(config.app.name.clone(), 4)
}

pub(crate) fn tls_runtime_from_config(config: &PlatformConfig) -> TlsRuntimeServices {
    TlsRuntime::from_config(&config.tls)
}

pub(crate) fn template_runtime_services() -> TemplateRuntimeServices {
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

pub(crate) fn i18n_runtime_from_config(
    config: &PlatformConfig,
    customer_catalogs: Vec<TranslationCatalog>,
) -> I18nRuntimeServices {
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
    let customer_catalogs_by_locale = customer_catalogs
        .into_iter()
        .map(|catalog| (catalog.locale().clone(), catalog))
        .collect::<std::collections::HashMap<_, _>>();
    let translations = TranslationRuntime::new(
        default_locale.clone(),
        supported_locales
            .iter()
            .cloned()
            .map(|locale| {
                merged_translation_catalog(
                    locale.clone(),
                    customer_catalogs_by_locale.get(&locale),
                )
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

fn merged_translation_catalog(
    locale: LocaleTag,
    customer_catalog: Option<&TranslationCatalog>,
) -> TranslationCatalog {
    let core_locale_key = davenda_i18n::MessageKey::new("core.locale").expect("static key");
    let mut messages = vec![(
        core_locale_key.clone(),
        locale.to_string(),
    )];
    if let Some(customer_catalog) = customer_catalog {
        messages.extend(
            customer_catalog
                .messages()
                .filter(|(key, _)| *key != &core_locale_key)
                .map(|(key, value)| (key.clone(), value.to_string())),
        );
    }
    TranslationCatalog::new(locale, messages).expect("merged translation catalog")
}

pub(crate) fn seo_runtime_from_config(config: &PlatformConfig) -> SeoRuntimeServices {
    SeoRuntimeServices {
        canonical_host: config.seo.canonical_host.clone(),
        emit_json_ld: config.seo.emit_json_ld,
        sitemap_enabled: config.seo.sitemap_enabled,
    }
}

pub(crate) fn a11y_runtime_services() -> A11yRuntimeServices {
    A11yRuntimeServices {
        navigation: NavigationContract::standard(),
        theme_baseline: ThemeAccessibilityContract::new(4.5, 3.0, 3.0, true, true)
            .expect("static baseline"),
    }
}

pub(crate) fn wasm_runtime_from_config(config: &PlatformConfig) -> WasmRuntimeServices {
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

pub(crate) fn tighten_runtime_limit(
    mut limits: ResourceLimits,
    max_runtime: Duration,
) -> ResourceLimits {
    if max_runtime < limits.max_runtime {
        limits.max_runtime = max_runtime;
    }

    limits
}

pub(crate) fn currency_for_locale(locale: &LocaleTag) -> CurrencyCode {
    let currency = match locale.as_str() {
        "fr-FR" => "EUR",
        _ => "GBP",
    };
    CurrencyCode::new(currency).expect("static currency")
}

pub(crate) fn timezone_for_locale(locale: &LocaleTag) -> TimeZoneId {
    let timezone = match locale.as_str() {
        "fr-FR" => "Europe/Paris",
        _ => "Europe/London",
    };
    TimeZoneId::new(timezone).expect("static timezone")
}
