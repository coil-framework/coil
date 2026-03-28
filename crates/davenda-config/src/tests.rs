use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

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
deployment = "distributed"
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
tenant_id = 101

[modules]
enabled = ["cms-pages", "admin-shell", "memberships", "events", "media-library"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false
[[wasm.outbound_http]]
integration = "crm"
endpoint = "https://crm.example.com/api"

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
fn parses_reference_config() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();

    assert_eq!(config.app.name, "showcase-events");
    assert_eq!(config.auth.tenant_id, 101);
    assert_eq!(config.tls.mode, TlsMode::Acme);
    assert_eq!(config.tls.challenge, Some(AcmeChallenge::Dns01));
    assert_eq!(config.database.driver, DatabaseDriver::Postgres);
    assert_eq!(
        config.database.url,
        Some(SecretRef::Env {
            var: "DATABASE_URL".to_string(),
        })
    );
    assert_eq!(config.cache.l1, CacheL1::Moka);
    assert_eq!(config.cache.l2, Some(DistributedCache::Redis));
    assert_eq!(config.http.session.store, SessionStore::Redis);
    assert_eq!(
        config.http.session_cookie.protection,
        CookieProtection::Signed
    );
    assert_eq!(config.wasm.outbound_http.len(), 1);
    assert_eq!(config.wasm.outbound_http[0].integration, "crm");
    assert_eq!(
        config.wasm.outbound_http[0].endpoint.as_str(),
        "https://crm.example.com/api"
    );
}

#[test]
fn rejects_default_locale_outside_supported_list() {
    let invalid = VALID_CONFIG.replace("default_locale = \"en-GB\"", "default_locale = \"de-DE\"");

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors.0.iter().any(|err| matches!(
                    err,
                    ConfigValidationError::DefaultLocaleNotSupported { .. }
                ))
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_dns_01_without_provider() {
    let invalid = VALID_CONFIG.replace("provider = \"cloudflare-dns\"\n", "");

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::MissingDnsAutomationProvider)
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_manifest_publishing_without_cdn_base_url() {
    let invalid = VALID_CONFIG.replace("cdn_base_url = \"https://cdn.example.com\"\n", "");

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(errors.0.contains(&ConfigValidationError::MissingCdnBaseUrl));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_single_node_sensitive_defaults_without_explicit_escape_hatch() {
    let invalid = VALID_CONFIG.replace(
        "default_class = \"public_upload\"",
        "default_class = \"local_only_sensitive\"",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(errors.0.contains(
                &ConfigValidationError::LocalOnlyStorageRequiresExplicitOptIn {
                    storage_class: StorageClass::LocalOnlySensitive,
                }
            ));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_explicit_single_node_escape_hatch_on_distributed_deployments() {
    let invalid = VALID_CONFIG.replace(
        "deployment = \"distributed\"",
        "deployment = \"distributed\"\nsingle_node_escape_hatch = \"explicit_single_node\"",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::LocalOnlyStorageRequiresSingleNodeDeployment)
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_session_store_without_matching_distributed_cache() {
    let invalid = VALID_CONFIG.replace("l2 = \"redis\"", "l2 = \"valkey\"");

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(errors.0.contains(
                &ConfigValidationError::SessionStoreRequiresDistributedCache {
                    store: SessionStore::Redis,
                    cache_backend: Some(DistributedCache::Valkey),
                }
            ));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_invalid_database_pool_sizing() {
    let overlay = r#"
[database]
min_connections = 8
max_connections = 4
"#;

    let error = PlatformConfig::from_toml_str_with_overlays(VALID_CONFIG, [overlay]).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::InvalidDatabasePoolSize {
                        min_connections: 8,
                        max_connections: 4,
                    })
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_invalid_trusted_proxy_entries() {
    let invalid = VALID_CONFIG.replace("10.0.0.0/8", "not-a-proxy");

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::InvalidTrustedProxy {
                        value: "not-a-proxy".to_string(),
                    })
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_duplicate_wasm_outbound_http_integrations() {
    let invalid = VALID_CONFIG.replace(
        "[[wasm.outbound_http]]\nintegration = \"crm\"\nendpoint = \"https://crm.example.com/api\"\n",
        "[[wasm.outbound_http]]\nintegration = \"crm\"\nendpoint = \"https://crm.example.com/api\"\n[[wasm.outbound_http]]\nintegration = \"crm\"\nendpoint = \"https://billing.example.com/api\"\n",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(errors.0.contains(
                &ConfigValidationError::DuplicateWasmOutboundHttpIntegration {
                    integration: "crm".to_string(),
                }
            ));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_non_http_wasm_outbound_http_endpoints() {
    let invalid = VALID_CONFIG.replace(
        "endpoint = \"https://crm.example.com/api\"",
        "endpoint = \"ftp://crm.example.com/api\"",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::InvalidWasmOutboundHttpScheme {
                        integration: "crm".to_string(),
                        scheme: "ftp".to_string(),
                    })
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_wasm_outbound_http_integrations_with_invalid_scheme() {
    let invalid = VALID_CONFIG.replace(
        "endpoint = \"https://crm.example.com/api\"",
        "endpoint = \"ftp://crm.example.com/api\"",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::InvalidWasmOutboundHttpScheme {
                        integration: "crm".to_string(),
                        scheme: "ftp".to_string(),
                    })
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn rejects_wasm_outbound_http_integrations_with_credentials() {
    let invalid = VALID_CONFIG.replace(
        "endpoint = \"https://crm.example.com/api\"",
        "endpoint = \"https://user:secret@crm.example.com/api\"",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(
                errors
                    .0
                    .contains(&ConfigValidationError::WasmOutboundHttpHasCredentials {
                        integration: "crm".to_string(),
                    })
            );
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn parses_cookie_protection_overrides() {
    let overlay = r#"
[http.session_cookie]
protection = "encrypted"
"#;

    let config = PlatformConfig::from_toml_str_with_overlays(VALID_CONFIG, [overlay]).unwrap();

    assert_eq!(
        config.http.session_cookie.protection,
        CookieProtection::Encrypted
    );
    assert_eq!(
        config.http.flash_cookie.protection,
        CookieProtection::Signed
    );
}

#[test]
fn overlay_toml_can_override_nested_values() {
    let overlay = r#"
[cache]
l2 = "valkey"

[http.session]
store = "valkey"

[seo]
canonical_host = "preview.example.com"
"#;

    let config = PlatformConfig::from_toml_str_with_overlays(VALID_CONFIG, [overlay]).unwrap();

    assert_eq!(config.cache.l2, Some(DistributedCache::Valkey));
    assert_eq!(config.seo.canonical_host, "preview.example.com");
}

#[test]
fn rendered_effective_config_contains_applied_values() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let rendered = config.render_effective_toml().unwrap();

    assert!(rendered.contains("showcase-events"));
    assert!(rendered.contains("platform-default-auth"));
    assert!(rendered.contains("cdn.example.com"));
}

#[test]
fn trusted_proxies_gate_forwarded_metadata_trust() {
    use std::net::SocketAddr;

    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();

    assert!(
        config
            .server
            .trusts_forwarded_headers(Some(&SocketAddr::from(([10, 0, 0, 8], 443,))))
    );
    assert!(
        !config
            .server
            .trusts_forwarded_headers(Some(&SocketAddr::from(([192, 168, 1, 8], 443,))))
    );
    assert!(!config.server.trusts_forwarded_headers(None));
}

#[test]
fn customer_bootstrap_manifest_validates_aligned_runtime_config() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let manifest = CustomerAppBootstrapManifest::new(
        "showcase-events",
        "en-GB",
        vec!["en-GB".to_string(), "fr-FR".to_string()],
        true,
        Vec::new(),
        "platform-default-auth",
        vec![
            "cms-pages".to_string(),
            "admin-shell".to_string(),
            "memberships".to_string(),
            "events".to_string(),
            "media-library".to_string(),
        ],
        Vec::new(),
        "www.example.com",
    );

    manifest.validate_runtime_config_alignment(&config).unwrap();
}

#[test]
fn customer_bootstrap_manifest_reports_module_drift() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let manifest = CustomerAppBootstrapManifest::new(
        "showcase-events",
        "en-GB",
        vec!["en-GB".to_string(), "fr-FR".to_string()],
        true,
        Vec::new(),
        "platform-default-auth",
        vec!["cms-pages".to_string(), "memberships".to_string()],
        Vec::new(),
        "www.example.com",
    );

    let error = manifest
        .validate_runtime_config_alignment(&config)
        .unwrap_err();

    match error {
        CustomerAppBootstrapManifestError::ModulesMismatch {
            manifest_only,
            configured_only,
        } => {
            assert!(manifest_only.is_empty());
            assert_eq!(
                configured_only,
                vec![
                    "admin-shell".to_string(),
                    "events".to_string(),
                    "media-library".to_string(),
                ]
            );
        }
        other => panic!("expected modules mismatch, got {other:?}"),
    }
}

#[test]
fn customer_bootstrap_manifest_loads_from_file() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir =
        std::env::temp_dir().join(format!("davenda-config-customer-bootstrap-{timestamp}"));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let manifest_path = temp_dir.join("app.toml");
    std::fs::write(
        &manifest_path,
        r#"
[app]
name = "showcase-events"

[domains]
canonical = "www.example.com"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms-pages", "admin-shell"]
"#,
    )
    .unwrap();

    let manifest = CustomerAppBootstrapManifest::from_file(&manifest_path).unwrap();

    assert_eq!(
        manifest.enabled_modules(),
        &["cms-pages".to_string(), "admin-shell".to_string()]
    );

    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::remove_dir(&temp_dir).unwrap();
}

#[test]
fn customer_bootstrap_manifest_accepts_explicit_sites_with_app_manifest_field_names() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "davenda-config-customer-bootstrap-sites-{timestamp}"
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let manifest_path = temp_dir.join("app.toml");
    std::fs::write(
        &manifest_path,
        r#"
[app]
name = "showcase-events"

[domains]
canonical = "www.example.com"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[[sites]]
id = "storefront"
display_name = "Showcase Storefront"
canonical_domain = "shop.example.com"
additional_domains = ["www.shop.example.com"]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms-pages", "admin-shell"]
"#,
    )
    .unwrap();

    let manifest = CustomerAppBootstrapManifest::from_file(&manifest_path).unwrap();

    assert_eq!(
        manifest.enabled_modules(),
        &["cms-pages".to_string(), "admin-shell".to_string()]
    );

    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::remove_dir(&temp_dir).unwrap();
}

#[test]
fn customer_bootstrap_manifest_parses_translation_catalogs() {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "davenda-config-customer-bootstrap-translations-{timestamp}"
    ));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let manifest_path = temp_dir.join("app.toml");
    std::fs::write(
        &manifest_path,
        r#"
[app]
name = "showcase-events"

[domains]
canonical = "www.example.com"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[translations]

[[translations.catalogs]]
locale = "en-GB"
path = "translations/en-GB.toml"

[[translations.catalogs]]
locale = "fr-FR"
path = "translations/fr-FR.toml"

[auth]
package = "platform-default-auth"

[modules]
enabled = ["cms-pages", "admin-shell"]
"#,
    )
    .unwrap();

    let manifest = CustomerAppBootstrapManifest::from_file(&manifest_path).unwrap();

    assert_eq!(manifest.translation_catalogs().len(), 2);
    assert_eq!(manifest.translation_catalogs()[0].locale(), "en-GB");
    assert_eq!(manifest.translation_catalogs()[0].path(), "translations/en-GB.toml");

    std::fs::remove_file(&manifest_path).unwrap();
    std::fs::remove_dir(&temp_dir).unwrap();
}

#[test]
fn parses_site_only_config_without_top_level_locale_or_canonical_defaults() {
    let config = PlatformConfig::from_toml_str(
        r#"
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
deployment = "distributed"
object_store = "s3"
local_root = "/var/lib/platform"

[cache]
l1 = "moka"
l2 = "redis"

[[sites]]
id = "storefront"
display_name = "Showcase Storefront"
canonical_host = "shop.example.com"
hosts = ["www.example.com"]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
localized_routes = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms-pages", "admin-shell", "memberships", "events", "media-library"]

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
"#,
    )
    .unwrap();

    assert_eq!(config.canonical_host_for_site(None), "shop.example.com");
    assert_eq!(config.default_locale_for_site(None), "en-GB");
    assert_eq!(
        config.supported_locales_for_site(None),
        &["en-GB".to_string(), "fr-FR".to_string()]
    );
    assert!(config.localized_routes_for_site(None));
}

#[test]
fn explicit_sites_override_top_level_localized_route_policy() {
    let config = PlatformConfig::from_toml_str(
        &format!(
            "{}\n[[sites]]\nid = \"storefront\"\ndisplay_name = \"Showcase Storefront\"\ncanonical_host = \"shop.example.com\"\nhosts = [\"www.example.com\"]\ndefault_locale = \"en-GB\"\nsupported_locales = [\"en-GB\", \"fr-FR\"]\nlocalized_routes = false\n",
            VALID_CONFIG
        ),
    )
    .unwrap();

    assert!(!config.localized_routes_for_site(Some("storefront")));
    assert!(!config.localized_routes_for_site(None));
}

#[test]
fn site_lookup_accepts_browser_host_headers_with_ports() {
    let config = PlatformConfig::from_toml_str(
        &format!(
            "{}\n[[sites]]\nid = \"storefront\"\ndisplay_name = \"Showcase Storefront\"\ncanonical_host = \"shop.example.com\"\nhosts = [\"www.example.com\", \"localhost\", \"127.0.0.1\"]\ndefault_locale = \"en-GB\"\nsupported_locales = [\"en-GB\", \"fr-FR\"]\nlocalized_routes = false\n",
            VALID_CONFIG
        ),
    )
    .unwrap();

    assert_eq!(
        config
            .site_for_host("shop.example.com:443")
            .map(|site| site.id.as_str()),
        Some("storefront")
    );
    assert_eq!(
        config
            .site_for_host("localhost:58080")
            .map(|site| site.id.as_str()),
        Some("storefront")
    );
    assert_eq!(
        config
            .site_for_host("127.0.0.1:58080")
            .map(|site| site.id.as_str()),
        Some("storefront")
    );
}
