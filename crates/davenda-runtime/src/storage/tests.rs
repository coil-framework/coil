use super::*;
use crate::RuntimeBuilder;
use davenda_auth::DefaultAuthModelPackage;
use davenda_config::{PlatformConfig, StorageClass};
use davenda_storage::StoragePolicyOverride;
use davenda_storage::execution::{ObjectStoreClientConfig, StorageDeliveryLocation};
use std::fs;
use std::path::PathBuf;

#[test]
fn publication_gate_reports_missing_conditions() {
    let gate = ManagedAssetPublicationGate {
        can_publish: true,
        can_replace: false,
        can_manage_storage: true,
        public_delivery_enabled: false,
    };

    assert!(!gate.can_publish_publicly());
    let error = gate
        .ensure_public_delivery_allowed("asset-hero")
        .unwrap_err();
    assert_eq!(
        error,
        RuntimeStorageError::PublicationAuthorizationDenied {
            asset_id: "asset-hero".to_string(),
            reason: "asset.replace, published public delivery state".to_string(),
        }
    );
}

fn test_config() -> PlatformConfig {
    PlatformConfig::from_toml_str(
        r#"
[app]
name = "davenda-runtime-storage-tests"
environment = "development"

[server]
bind = "127.0.0.1:3000"
trusted_proxies = []

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
mode = "external"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
local_root = "/tmp/davenda-runtime-storage-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en"
supported_locales = ["en"]
fallback_locale = "en"
localized_routes = false

[seo]
canonical_host = "example.test"
emit_json_ld = false

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms"]

[wasm]
directory = "/tmp/davenda-runtime-storage-tests"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
cdn_base_url = "https://cdn.example.test"
"#,
    )
    .unwrap()
}

fn unique_test_root() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("davenda-runtime-storage-tests-{nanos}"))
}

#[test]
fn storage_host_plans_public_delivery_and_executes_local_escape_hatch_storage() {
    let root = unique_test_root();
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    let mut config = test_config();
    config.storage.local_root = root.display().to_string();
    config.wasm.directory = root.display().to_string();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let host = plan.storage_host();

    let object_plan = host
        .plan_write(
            davenda_storage::StoragePlanRequest::new("uploads/catalog/item.jpg")
                .with_storage_class(StorageClass::PublicUpload),
        )
        .unwrap();
    assert!(matches!(
        host.delivery_location(&object_plan).unwrap(),
        StorageDeliveryLocation::PublicCdn { .. }
    ));

    let local_plan = host
        .plan_single_node_escape_hatch_write(
            davenda_storage::StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(StorageClass::PrivateShared)
                .with_override(StoragePolicyOverride::force_single_node_escape_hatch()),
        )
        .unwrap();
    let local_write = host.execute_write(&local_plan, b"local-bytes").unwrap();
    assert_eq!(local_write.path, root.join("secure/reports/march.csv"));
    assert_eq!(
        host.execute_read(&local_plan).unwrap().bytes,
        b"local-bytes"
    );
    assert_eq!(
        host.delivery_location(&local_plan).unwrap(),
        StorageDeliveryLocation::LocalPath {
            path: root.join("secure/reports/march.csv"),
        }
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn storage_host_generates_signed_urls_for_private_object_store_assets() {
    let plan = RuntimeBuilder::new(test_config(), DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let host = plan.storage_host_with_object_store(Some(
        ObjectStoreClientConfig::new("runtime", "us-east-1")
            .unwrap()
            .with_endpoint_url("https://storage.example.test")
            .unwrap()
            .with_static_credentials("runtime-access", "runtime-secret")
            .unwrap()
            .with_signed_url_ttl_secs(900),
    ));

    let private_plan = host
        .plan_write(
            davenda_storage::StoragePlanRequest::new("secure/reports/april.csv")
                .with_storage_class(StorageClass::PrivateShared),
        )
        .unwrap();

    match host.delivery_location(&private_plan).unwrap() {
        StorageDeliveryLocation::SignedObject {
            object_key,
            signed_url,
            expires_at_unix_seconds,
        } => {
            assert_eq!(object_key, "secure/reports/april.csv");
            assert!(signed_url.contains("X-Amz-Algorithm=AWS4-HMAC-SHA256"));
            assert!(expires_at_unix_seconds > 0);
        }
        other => panic!("expected signed delivery, got {other:?}"),
    }
}
