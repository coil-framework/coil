use super::*;

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
fn rejects_local_only_defaults_without_explicit_escape_hatch() {
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
fn rejects_explicit_local_only_on_distributed_deployments() {
    let invalid = VALID_CONFIG.replace(
        "deployment = \"distributed\"",
        "deployment = \"distributed\"\nsingle_node_escape_hatch = \"explicit_single_node\"",
    );

    let error = PlatformConfig::from_toml_str(&invalid).unwrap_err();

    match error {
        ConfigError::Validation(errors) => {
            assert!(errors
                .0
                .contains(&ConfigValidationError::LocalOnlyStorageRequiresSingleNodeDeployment));
        }
        other => panic!("expected validation error, got {other:?}"),
    }
}

#[test]
fn accepts_legacy_local_only_alias_for_single_node_escape_hatch() {
    let config = VALID_CONFIG.replace(
        "deployment = \"distributed\"",
        "deployment = \"single_node\"\nlocal_only = \"explicit_single_node\"",
    );

    let parsed = PlatformConfig::from_toml_str(&config).unwrap();

    assert_eq!(
        parsed.storage.single_node_escape_hatch,
        LocalOnlyStorageMode::ExplicitSingleNode
    );
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
