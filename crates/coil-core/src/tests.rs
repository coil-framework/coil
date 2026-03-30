use super::*;
use coil_auth::DefaultAuthModelPackage;
use coil_cache::DistributedCacheBackend;
use coil_config::PlatformConfig;
use coil_i18n::MessageKey;
use coil_template::TemplateNamespace;
use coil_wasm::ExtensionPointKind;

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
name = "coil_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "coil_flash"
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
local_root = "/var/lib/coil"

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
package = "coil-default-auth"
explain_api = false
tenant_id = 101

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
    assert!(ids.contains(&"core.customer_plugins"));
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
        "coil_session"
    );
    assert_eq!(bootstrap.browser.csrf.field_name, "_csrf");
    assert!(bootstrap.cli.customer_app == "showcase-events");
    assert_eq!(bootstrap.cli.baseline_command_count, 4);
    assert_eq!(bootstrap.data.driver, coil_config::DatabaseDriver::Postgres);
    assert_eq!(bootstrap.data.schema, "public");
    assert_eq!(bootstrap.data.migrations_table, "_coil_migrations");
    assert_eq!(bootstrap.jobs.backend, coil_config::JobBackend::Redis);
    assert_eq!(bootstrap.jobs.topology.work_queue.as_str(), "jobs.work");
    assert_eq!(
        bootstrap.jobs.topology.domain_events_queue.as_str(),
        "jobs.domain-events"
    );
    assert_eq!(bootstrap.tls.mode, TlsMode::Acme);
    assert_eq!(
        bootstrap.tls.provider,
        Some(coil_tls::CertificateProviderKind::CloudflareDns)
    );
    assert_eq!(
        bootstrap.tls.challenge,
        Some(coil_tls::ChallengeStrategy::Dns01)
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
        ])
        .with_jobs(vec![JobContract::new(
            "cms.publish-scheduled",
            JobTriggerKind::Scheduled,
            true,
            "Publishes scheduled pages",
        )])
        .with_event_subscriptions(vec![EventSubscription::new(
            "cms.page.publish-requested",
            Some("cms.publish-scheduled"),
            "Schedules page publication",
        )]);

    assert!(validate_module_capabilities(&package, &manifest).is_ok());

    let invalid = ModuleManifest::new("cms-pages")
        .with_required_capabilities(vec![Capability::CmsPageRead])
        .with_capability_contracts(vec![CapabilityContract::required(
            Capability::CmsPageRead,
            ["page"],
        )])
        .with_event_subscriptions(vec![EventSubscription::new(
            "cms.page.publish-requested",
            Some("cms.publish-scheduled"),
            "Schedules page publication",
        )]);
    assert_eq!(
        validate_module_capabilities(&package, &invalid).unwrap_err(),
        CapabilityValidationError::UnknownSubscriptionJob {
            module: "cms-pages".to_string(),
            event: "cms.page.publish-requested".to_string(),
            job: "cms.publish-scheduled".to_string(),
        }
    );

    let invalid_search = ModuleManifest::new("search")
        .with_optional_capabilities(vec![Capability::CmsPageRead])
        .with_capability_contracts(vec![CapabilityContract::optional(
            Capability::CmsPageRead,
            ["page"],
        )])
        .with_search_contributions(vec![SearchIndexContribution::new(
            "search.pages",
            SearchDocumentKind::Page,
            SearchVisibility::Public,
            false,
            vec![SearchFieldContribution::new(
                "title",
                "title",
                SearchFieldRole::Title,
                true,
                true,
            )],
            vec![SearchInvalidationRule::new(
                SearchInvalidationTrigger::Published,
                "page published",
            )],
            SearchRebuildStrategy::OnInvalidate,
        )]);
    assert_eq!(
        validate_module_capabilities(&package, &invalid_search).unwrap_err(),
        CapabilityValidationError::InvalidOperationalContribution {
            module: "search".to_string(),
            kind: "search contribution",
            id: "search.pages".to_string(),
            reason: "public search indexes must require publication state".to_string(),
        }
    );
}

#[test]
fn bootstrap_core_services_merges_customer_translation_catalogs() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let bootstrap = bootstrap_core_services_with_translation_catalogs(
        &config,
        vec![
            TranslationCatalog::new(
                LocaleTag::new("fr-FR").unwrap(),
                vec![(
                    MessageKey::new("checkout.title").unwrap(),
                    "Paiement".to_string(),
                )],
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let context = bootstrap.i18n.request_context(Some("fr-FR"));
    assert_eq!(
        bootstrap
            .i18n
            .translations
            .translate(&context, &MessageKey::new("checkout.title").unwrap())
            .unwrap(),
        "Paiement"
    );
    assert_eq!(
        bootstrap
            .i18n
            .translations
            .translate(&context, &MessageKey::new("core.locale").unwrap())
            .unwrap(),
        "fr-FR"
    );
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
        &[
            "core.auth",
            "core.data",
            "core.data.migrations",
            "core.jobs",
        ],
    )
    .unwrap_err();
    assert_eq!(
        missing_dependency,
        ModuleInstallationError::MissingModuleDependency {
            module: "memberships".to_string(),
            dependency: "commerce".to_string(),
        }
    );

    assert!(
        validate_module_installation(
            &manifest,
            &["commerce".to_string(), "memberships".to_string()],
            &[
                "core.auth",
                "core.data",
                "core.data.migrations",
                "core.jobs"
            ],
        )
        .is_ok()
    );
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
fn encrypted_cookie_round_trips_and_rejects_tampering() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let bootstrap = bootstrap_core_services(&config).unwrap();
    let sealer = CookieSealer::new(CookiePolicy {
        protection: CookieProtection::Encrypted,
        ..bootstrap.browser.sessions.flash_cookie.clone()
    });
    let secret = b"fedcba9876543210fedcba9876543210";

    let sealed = sealer.seal(secret, "flash:welcome-back").unwrap();
    assert_eq!(sealer.open(secret, &sealed).unwrap(), "flash:welcome-back");

    let mut tampered = sealed.clone();
    let last = tampered.pop().unwrap();
    tampered.push(if last == 'A' { 'B' } else { 'A' });
    assert!(sealer.open(secret, &tampered).is_err());
}

#[test]
fn cookie_primitives_enforce_declared_protection_mode() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let bootstrap = bootstrap_core_services(&config).unwrap();
    let signed = bootstrap.browser.sessions.session_cookie.clone();
    let encrypted = CookiePolicy {
        protection: CookieProtection::Encrypted,
        ..bootstrap.browser.sessions.flash_cookie.clone()
    };
    let secret = b"0123456789abcdef0123456789abcdef";

    assert_eq!(
        CookieSigner::new(encrypted.clone())
            .sign(secret, "value")
            .unwrap_err(),
        BrowserSecurityError::UnexpectedCookieProtection {
            expected: CookieProtection::Signed,
            actual: CookieProtection::Encrypted,
        }
    );
    assert_eq!(
        CookieSealer::new(signed).seal(secret, "value").unwrap_err(),
        BrowserSecurityError::UnexpectedCookieProtection {
            expected: CookieProtection::Encrypted,
            actual: CookieProtection::Signed,
        }
    );
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
