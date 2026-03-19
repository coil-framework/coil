use super::*;

#[test]
fn rejects_invalid_policy_combinations() {
    let policy = StoragePolicy::new(
        DeliveryMode::PublicCdn,
        SyncMode::LocalOnly,
        Sensitivity::Public,
    );

    assert_eq!(
        policy.validate(),
        Err(StoragePolicyError::InvalidCombination {
            detail: "public_cdn delivery requires object_store sync".to_string(),
        })
    );
}

#[test]
fn public_asset_policies_are_public_delivery_eligible() {
    assert!(StoragePolicy::public_asset().is_public_delivery_eligible());
    assert!(!StoragePolicy::private_shared().is_public_delivery_eligible());
    assert!(!StoragePolicy::single_node_sensitive().is_public_delivery_eligible());
}

#[test]
fn public_delivery_invariant_is_centralized_on_storage_plans() {
    let planner = StoragePlanner::from_config(&test_config());
    let plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("uploads/catalog/item.jpg")
                .with_storage_class(davenda_config::StorageClass::PublicUpload),
        )
        .expect("public upload plan");

    assert!(plan.public_delivery_eligible());
    assert_eq!(plan.ensure_public_delivery_allowed(), Ok(()));

    let private_plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(davenda_config::StorageClass::PrivateShared),
        )
        .expect("private upload plan");

    assert!(!private_plan.public_delivery_eligible());
    assert_eq!(
        private_plan.ensure_public_delivery_allowed(),
        Err(StoragePlanningError::PublicDeliveryNotEligible {
            logical_path: "secure/reports/march.csv".to_string(),
            policy: StoragePolicy::private_shared(),
        })
    );
}

#[test]
fn resolves_most_specific_path_policy() {
    let policies = StoragePolicySet::default()
        .with_rule(
            PathPolicyRule::new(
                "uploads",
                Some(davenda_config::StorageClass::PrivateShared),
                StoragePolicy::private_shared(),
            )
            .expect("valid root uploads rule"),
        )
        .with_rule(
            PathPolicyRule::new(
                "uploads/marketing",
                Some(davenda_config::StorageClass::PublicUpload),
                StoragePolicy::public_upload(),
            )
            .expect("valid nested marketing rule")
            .with_object_prefix("public/marketing")
            .expect("valid object prefix"),
        );

    let resolved = policies
        .resolve(
            davenda_config::StorageClass::PrivateShared,
            "uploads/marketing/brochure.pdf",
            None,
        )
        .expect("path policy resolves");

    assert_eq!(
        resolved.storage_class,
        davenda_config::StorageClass::PublicUpload
    );
    assert_eq!(resolved.policy, StoragePolicy::public_upload());
    assert_eq!(resolved.object_prefix.as_deref(), Some("public/marketing"));
    assert_eq!(
        resolved.matched_rule_prefix.as_deref(),
        Some("uploads/marketing")
    );
}

#[test]
fn object_store_policies_plan_write_through_storage() {
    let config = test_config();
    let planner = StoragePlanner::new(
        StorageTopology::from_config(&config),
        StoragePolicySet::default().with_rule(
            PathPolicyRule::new(
                "uploads/marketing",
                Some(davenda_config::StorageClass::PublicUpload),
                StoragePolicy::public_upload(),
            )
            .expect("valid marketing rule")
            .with_object_prefix("public")
            .expect("valid object prefix"),
        ),
    );

    let plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("uploads/marketing/hero.webp")
                .with_storage_class(davenda_config::StorageClass::PublicUpload),
        )
        .expect("public uploads should plan against object storage");

    assert_eq!(plan.durable_store, DurableStore::ObjectStore);
    assert_eq!(plan.deployment_scope, StorageDeploymentScope::Scalable);
    assert_eq!(
        plan.object_key.as_deref(),
        Some("public/uploads/marketing/hero.webp")
    );
    assert_eq!(plan.local_path, None);
    assert_eq!(
        plan.primary_write_target()
            .expect("primary write target")
            .backend,
        StorageBackendKind::S3Compatible
    );
}

#[test]
fn scalable_planner_rejects_the_single_node_escape_hatch() {
    let planner = StoragePlanner::from_config(&test_config());

    let error = planner
        .plan_scalable_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(davenda_config::StorageClass::PrivateShared)
                .with_override(StoragePolicyOverride::force_single_node_escape_hatch()),
        )
        .expect_err("scalable planning should not accept the single-node escape hatch");

    assert_eq!(
        error,
        StoragePlanningError::SingleNodeEscapeHatchRequested {
            logical_path: "secure/reports/march.csv".to_string(),
            policy: StoragePolicy::single_node_sensitive(),
        }
    );
}

#[test]
fn single_node_escape_hatch_override_keeps_sensitive_files_on_server() {
    let planner = StoragePlanner::from_config(&test_config());

    let plan = planner
        .single_node_escape_hatch()
        .plan_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(davenda_config::StorageClass::PrivateShared)
                .with_override(StoragePolicyOverride::force_single_node_escape_hatch()),
        )
        .expect("local-only override should succeed");

    assert_eq!(plan.policy, StoragePolicy::single_node_sensitive());
    assert_eq!(plan.durable_store, DurableStore::LocalDisk);
    assert_eq!(
        plan.deployment_scope,
        StorageDeploymentScope::SingleNodeOnly
    );
    assert!(plan.requires_single_node());
    assert_eq!(
        plan.local_path.as_deref(),
        Some("var/davenda/storage/secure/reports/march.csv")
    );
    assert_eq!(plan.object_key, None);
}

#[test]
fn single_node_escape_hatch_override_is_rejected_for_distributed_deployments() {
    let mut config = test_config();
    config.storage.deployment = davenda_config::StorageDeployment::Distributed;
    let planner = StoragePlanner::from_config(&config);

    let error = planner
        .single_node_escape_hatch()
        .plan_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(davenda_config::StorageClass::PrivateShared)
                .with_override(StoragePolicyOverride::force_single_node_escape_hatch()),
        )
        .expect_err("local-only override should be rejected on distributed deployments");

    assert_eq!(
        error,
        StoragePlanningError::SingleNodeEscapeHatchNotAllowedForDeployment {
            logical_path: "secure/reports/march.csv".to_string(),
            policy: StoragePolicy::single_node_sensitive(),
            deployment: davenda_config::StorageDeployment::Distributed,
            single_node_escape_hatch: davenda_config::SingleNodeStorageMode::ExplicitSingleNode,
        }
    );
}

#[test]
fn rejects_parent_traversal() {
    let planner = StoragePlanner::from_config(&test_config());

    let error = planner
        .plan_scalable_write(StoragePlanRequest::new("../secrets.txt"))
        .expect_err("parent traversal must be rejected");

    assert_eq!(
        error,
        StoragePlanningError::Policy(StoragePolicyError::ParentTraversal {
            path: "../secrets.txt".to_string(),
        })
    );
}

#[test]
fn object_store_sync_requires_backend_configuration() {
    let mut config = test_config();
    config.storage.object_store = None;
    let planner = StoragePlanner::from_config(&config);

    let error = planner
        .plan_scalable_write(
            StoragePlanRequest::new("uploads/catalog/item.jpg")
                .with_storage_class(davenda_config::StorageClass::PublicUpload),
        )
        .expect_err("public uploads should require object storage");

    assert_eq!(
        error,
        StoragePlanningError::ObjectStoreRequired {
            logical_path: "uploads/catalog/item.jpg".to_string(),
            policy: StoragePolicy::public_upload(),
        }
    );
}

fn test_config() -> davenda_config::PlatformConfig {
    davenda_config::PlatformConfig::from_toml_str(
        r#"
[app]
name = "davenda"
environment = "development"

[server]
bind = "127.0.0.1:3000"

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
default_class = "private_shared"
deployment = "single_node"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
local_root = "var/davenda/storage"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "en-US"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "https://example.com"
emit_json_ld = true
sitemap_enabled = true

[auth]
package = "platform-default-auth"
explain_api = true
tenant_id = 101

[modules]
enabled = ["davenda-cms", "davenda-commerce"]

[wasm]
directory = "wasm"
default_time_limit_ms = 25
allow_network = false

[jobs]
backend = "redis"
retry_limit = 5

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#,
    )
    .expect("valid test config")
}
