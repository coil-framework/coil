use super::*;
use crate::{PathPolicyRule, StoragePlanRequest, StoragePlanner, StoragePolicy, StoragePolicySet};
use davenda_config::{PlatformConfig, StorageClass};
use std::path::PathBuf;

fn test_config() -> PlatformConfig {
    PlatformConfig::from_toml_str(
        r#"
[app]
name = "davenda-storage-tests"
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
local_root = "/tmp/davenda-storage-tests"
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
directory = "/tmp/davenda-storage-tests"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#,
    )
    .unwrap()
}

fn planner() -> StoragePlanner {
    StoragePlanner::new(
        crate::StorageTopology::from_config(&test_config()),
        StoragePolicySet::default().with_rule(
            PathPolicyRule::new(
                "uploads/marketing",
                Some(StorageClass::PublicUpload),
                StoragePolicy::public_upload(),
            )
            .unwrap()
            .with_object_prefix("public/marketing")
            .unwrap(),
        ),
    )
}

#[test]
fn object_store_execution_writes_reads_and_resolves_delivery_locations() {
    let planner = planner();
    let executor = StorageExecutor::from_topology(planner.topology());
    let public_plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("uploads/marketing/hero.webp")
                .with_storage_class(StorageClass::PublicUpload),
        )
        .unwrap();

    let receipt = executor.execute_write(&public_plan, b"hero-bytes").unwrap();
    let object_key = public_plan.object_key.as_deref().unwrap();
    assert_eq!(receipt.target.backend, StorageBackendKind::S3Compatible);
    assert!(
        receipt
            .path
            .ends_with(format!("object-store/{}", object_key))
    );
    assert_eq!(
        executor.execute_read(&public_plan).unwrap().bytes,
        b"hero-bytes"
    );
    assert_eq!(
        executor
            .delivery_location(&public_plan, Some("https://cdn.example.com"))
            .unwrap(),
        StorageDeliveryLocation::PublicCdn {
            public_url: format!("https://cdn.example.com/{}", object_key),
            object_key: object_key.to_string(),
        }
    );

    let private_plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(StorageClass::PrivateShared),
        )
        .unwrap();
    let private_object_key = private_plan.object_key.as_deref().unwrap();
    assert_eq!(
        executor.delivery_location(&private_plan, None).unwrap(),
        StorageDeliveryLocation::SignedObject {
            object_key: private_object_key.to_string(),
        }
    );
}

#[test]
fn local_disk_execution_writes_reads_and_resolves_local_delivery() {
    let mut config = test_config();
    config.storage.object_store = None;
    let planner = StoragePlanner::from_config(&config);
    let executor = StorageExecutor::from_topology(planner.topology());
    let plan = planner
        .single_node_escape_hatch()
        .plan_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(StorageClass::PrivateShared)
                .with_override(crate::StoragePolicyOverride::force_single_node_escape_hatch()),
        )
        .unwrap();

    let receipt = executor.execute_write(&plan, b"sensitive-bytes").unwrap();
    assert_eq!(receipt.target.backend, StorageBackendKind::LocalDisk);
    assert_eq!(receipt.bytes_written, "sensitive-bytes".len() as u64);
    assert_eq!(
        receipt.path,
        PathBuf::from("/tmp/davenda-storage-tests/secure/reports/march.csv")
    );
    assert_eq!(
        executor.execute_read(&plan).unwrap().bytes,
        b"sensitive-bytes"
    );
    assert_eq!(
        executor.delivery_location(&plan, None).unwrap(),
        StorageDeliveryLocation::LocalPath {
            path: PathBuf::from("/tmp/davenda-storage-tests/secure/reports/march.csv"),
        }
    );
}
