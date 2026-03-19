use super::*;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn tag(value: &str) -> InvalidationTag {
    InvalidationTag::new(value).unwrap()
}

fn persistent_namespace(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        "{prefix}-{}-{timestamp}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[derive(Clone)]
struct SharedCacheRuntimeHarness {
    runtime: Arc<dyn DistributedCacheRuntime>,
}

impl SharedCacheRuntimeHarness {
    fn new(runtime: Arc<dyn DistributedCacheRuntime>) -> Self {
        Self { runtime }
    }
}

impl DistributedCacheRuntime for SharedCacheRuntimeHarness {
    fn insert(&self, entry: CacheEntry) {
        self.runtime.insert(entry);
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.runtime.lookup(key, now)
    }

    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.runtime.invalidate(tags)
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        self.runtime.begin_fill(key, mode, holder)
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.runtime.complete_fill(lease)
    }

    fn metrics(&self) -> CacheMetrics {
        self.runtime.metrics()
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[test]
fn variation_keys_are_stable_for_equivalent_scopes() {
    let left = CacheScope::public()
        .with_locale("en-GB")
        .unwrap()
        .with_tenant("tenant-a")
        .unwrap()
        .with_custom_variation("currency", "GBP")
        .unwrap()
        .with_custom_variation("channel", "web")
        .unwrap();
    let right = CacheScope::public()
        .with_custom_variation("channel", "web")
        .unwrap()
        .with_tenant("tenant-a")
        .unwrap()
        .with_custom_variation("currency", "GBP")
        .unwrap()
        .with_locale("en-GB")
        .unwrap();

    assert_eq!(left.variation_key(), right.variation_key());
    assert_eq!(
        left.variation_key().unwrap().as_str(),
        "tenant=tenant-a|locale=en-GB|x:channel=web|x:currency=GBP"
    );
}

#[test]
fn partition_keys_include_visibility_for_scope_isolation() {
    let public = CacheScope::public();
    let private = CacheScope::private();

    assert_eq!(
        public.cache_partition_key().unwrap().as_str(),
        "visibility=public"
    );
    assert_eq!(
        private.cache_partition_key().unwrap().as_str(),
        "visibility=private"
    );
    assert_ne!(public.cache_partition_key(), private.cache_partition_key());
}

#[test]
fn public_scope_rejects_user_and_session_variation() {
    assert_eq!(
        CacheScope::public().with_user("user-123").unwrap_err(),
        CacheModelError::PublicScopeCannotVaryByUser
    );
    assert_eq!(
        CacheScope::public().with_session("sess-123").unwrap_err(),
        CacheModelError::PublicScopeCannotVaryBySession
    );
}

#[test]
fn invalidation_tags_are_deduped_and_rendered_for_surrogate_headers() {
    let mut tags = InvalidationSet::new();
    tags.insert(tag("page:42"));
    tags.insert(tag("page:42"));
    tags.insert(tag("site:main"));

    assert_eq!(tags.len(), 2);
    assert_eq!(tags.header_value().as_deref(), Some("page:42 site:main"));
}

#[test]
fn topology_reports_cluster_and_local_coalescing_modes() {
    assert_eq!(
        CacheTopology::moka_only().request_coalescing_mode(),
        RequestCoalescingMode::Local
    );
    assert_eq!(
        CacheTopology::with_redis().request_coalescing_mode(),
        RequestCoalescingMode::Cluster
    );
    assert!(CacheTopology::with_valkey().supports_shared_invalidation());
    assert!(CacheTopology::with_valkey().supports_shared_coalescing());
}

#[test]
fn distributed_topology_enables_cluster_coalescing_and_l2_cache() {
    let topology = CacheTopology::with_redis();
    let planner = CachePlanner::new(topology);
    let app_policy = ApplicationCachePolicy::new(
        CacheScope::public()
            .with_site("main")
            .unwrap()
            .with_locale("en-GB")
            .unwrap(),
        FreshnessPolicy::new(Duration::from_secs(300), Some(Duration::from_secs(30))).unwrap(),
        InvalidationSet::from_tags([tag("page:42"), tag("nav:main")]),
    )
    .unwrap();
    let http_policy = HttpCachePolicy::new(
        CacheScope::public()
            .with_site("main")
            .unwrap()
            .with_locale("en-GB")
            .unwrap(),
        Some(FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(15))).unwrap()),
        ResponseValidators {
            etag: Some(EntityTag::new("etag-42").unwrap()),
            last_modified_unix_seconds: Some(1_763_000_000),
        },
        InvalidationSet::from_tags([tag("page:42"), tag("jsonld:page:42")]),
    )
    .unwrap();

    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.page").unwrap(),
                "page:42",
                http_policy,
            )
            .unwrap()
            .with_application_policy(app_policy),
        )
        .unwrap();

    let application = plan.application().unwrap();
    assert_eq!(
        application.key().to_string(),
        "cms.page:page:42|visibility=public|site=main|locale=en-GB"
    );
    assert_eq!(application.layers().l1, LocalCacheBackend::Moka);
    assert_eq!(
        application.layers().l2,
        Some(DistributedCacheBackend::Redis)
    );
    assert_eq!(application.coalescing(), RequestCoalescingMode::Cluster);
    assert_eq!(
        plan.http().cache_control(),
        "public, max-age=60, stale-while-revalidate=15"
    );
    assert!(plan.http().edge_cacheable());
    assert_eq!(
        plan.http().variation().unwrap().as_str(),
        "site=main|locale=en-GB"
    );
    assert_eq!(
        plan.http().surrogate_tags().header_value().as_deref(),
        Some("jsonld:page:42 page:42")
    );
}

#[test]
fn cache_runtime_new_keeps_distributed_topologies_local_until_explicitly_shared() {
    let runtime = CacheRuntime::new(CacheTopology::with_redis());

    assert_eq!(runtime.backend_kind(), CacheBackendKind::Redis);
    assert!(!runtime.backend_is_shared());
}

#[test]
#[allow(deprecated)]
fn compatibility_shared_shims_remain_local_only() {
    let client = DistributedCacheClient::shared(CacheBackendKind::Redis);
    let adapter = CacheBackendAdapter::shared(CacheTopology::with_redis());
    let emulated = DistributedCacheClient::emulated_shared_runtime(CacheBackendKind::Redis);
    let explicit = CacheBackendAdapter::with_shared_runtime(
        CacheTopology::with_redis(),
        Arc::new(SharedCacheRuntimeHarness::new(emulated.clone())),
    );
    let explicit_emulated =
        CacheBackendAdapter::with_shared_runtime(CacheTopology::with_redis(), emulated);

    assert!(!client.is_shared());
    assert!(!adapter.is_shared());
    assert!(explicit.is_shared());
    assert!(!explicit_emulated.is_shared());
}

#[test]
fn cache_runtime_new_uses_local_backend_for_single_node_topologies() {
    let mut left = CacheRuntime::new(CacheTopology::moka_only());
    let mut right = left.clone();

    assert_eq!(left.backend_kind(), CacheBackendKind::Local);
    assert!(!left.backend_is_shared());

    let planner = CachePlanner::new(CacheTopology::moka_only());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:test-shared",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    left.insert(
        plan.application().unwrap(),
        "<html>shared-test</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Miss);
}

#[test]
fn explicit_test_local_cache_runtime_keeps_state_isolated() {
    let mut left = CacheRuntime::local_for_testing(CacheTopology::moka_only());
    let mut right = left.clone();

    assert_eq!(left.backend_kind(), CacheBackendKind::Local);
    assert!(!left.backend_is_shared());

    let planner = CachePlanner::new(CacheTopology::moka_only());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:test-local",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    left.insert(
        plan.application().unwrap(),
        "<html>local-test</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Miss);
}

#[test]
fn planner_respects_explicit_coalescing_override() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "product:sku-1",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .with_request_coalescing_mode(RequestCoalescingMode::Disabled),
        )
        .unwrap();

    assert_eq!(
        plan.application().unwrap().coalescing(),
        RequestCoalescingMode::Disabled
    );
}

#[test]
fn local_cache_runtime_clones_do_not_share_state() {
    let planner = CachePlanner::new(CacheTopology::moka_only());
    let mut left = planner.local_for_testing();
    let mut right = left.clone();

    assert_eq!(left.backend_kind(), CacheBackendKind::Local);
    assert!(!left.backend_is_shared());

    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:1",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    left.insert(
        plan.application().unwrap(),
        "<html>local</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Miss);
}

#[test]
fn planner_runtime_defaults_are_local_even_for_distributed_topologies() {
    let planner = CachePlanner::new(CacheTopology::with_valkey());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:2",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    let mut left = planner.runtime();
    let mut right = planner.runtime();

    assert!(!left.backend_is_shared());
    assert_eq!(left.backend_kind(), CacheBackendKind::Valkey);

    left.insert(
        plan.application().unwrap(),
        "<html>shared</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Miss);
}

#[test]
fn distributed_planner_runtimes_share_backend_when_reusing_an_explicit_handle() {
    let topology = CacheTopology::with_valkey();
    let planner = CachePlanner::new(topology);
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:2",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    let shared_runtime = Arc::new(SharedCacheRuntimeHarness::new(
        DistributedCacheClient::emulated_shared_runtime(CacheBackendKind::Valkey),
    ));
    let adapter = CacheBackendAdapter::with_shared_runtime(topology, shared_runtime);
    let mut left = CacheRuntime::with_backend(topology, adapter.clone());
    let mut right = CacheRuntime::with_backend(topology, adapter);

    assert_eq!(left.backend_kind(), CacheBackendKind::Valkey);
    assert!(left.backend_is_shared());

    left.insert(
        plan.application().unwrap(),
        "<html>shared</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Fresh);
}

#[test]
fn persistent_shared_cache_runtime_shares_state_across_independent_clients() {
    let topology = CacheTopology::with_redis();
    let planner = CachePlanner::new(topology);
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:persistent",
                HttpCachePolicy::new(
                    CacheScope::public()
                        .with_site("main")
                        .unwrap()
                        .with_locale("en-GB")
                        .unwrap(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public()
                        .with_site("main")
                        .unwrap()
                        .with_locale("en-GB")
                        .unwrap(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    let namespace = persistent_namespace("cache");
    let left_runtime = DistributedCacheClient::test_only_persistent_shared_runtime(
        CacheBackendKind::Redis,
        namespace.clone(),
    );
    let right_runtime = DistributedCacheClient::test_only_persistent_shared_runtime(
        CacheBackendKind::Redis,
        namespace,
    );
    let left_adapter = CacheBackendAdapter::with_shared_runtime(topology, left_runtime);
    let right_adapter = CacheBackendAdapter::with_shared_runtime(topology, right_runtime);
    let mut left = CacheRuntime::with_backend(topology, left_adapter);
    let mut right = CacheRuntime::with_backend(topology, right_adapter);

    left.insert(
        plan.application().unwrap(),
        "<html>persistent</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Fresh);
    assert!(right.backend_is_shared());
}

#[test]
fn persistent_shared_cache_runtime_isolated_across_namespaces() {
    let topology = CacheTopology::with_redis();
    let planner = CachePlanner::new(topology);
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:isolated",
                HttpCachePolicy::new(
                    CacheScope::public()
                        .with_site("main")
                        .unwrap()
                        .with_locale("en-GB")
                        .unwrap(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public()
                        .with_site("main")
                        .unwrap()
                        .with_locale("en-GB")
                        .unwrap(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    let left_runtime = DistributedCacheClient::test_only_persistent_shared_runtime(
        CacheBackendKind::Redis,
        persistent_namespace("cache-left"),
    );
    let right_runtime = DistributedCacheClient::test_only_persistent_shared_runtime(
        CacheBackendKind::Redis,
        persistent_namespace("cache-right"),
    );
    let left_adapter = CacheBackendAdapter::with_shared_runtime(topology, left_runtime);
    let right_adapter = CacheBackendAdapter::with_shared_runtime(topology, right_runtime);
    let mut left = CacheRuntime::with_backend(topology, left_adapter);
    let mut right = CacheRuntime::with_backend(topology, right_adapter);

    left.insert(
        plan.application().unwrap(),
        "<html>left</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let left_lookup = left.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    let right_lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );

    assert_eq!(left_lookup.state, CacheLookupState::Fresh);
    assert_eq!(right_lookup.state, CacheLookupState::Miss);
    assert!(left.backend_is_shared());
    assert!(right.backend_is_shared());
}

#[test]
fn distributed_planner_local_runtimes_do_not_share_state_without_explicit_client_wiring() {
    let planner = CachePlanner::new(CacheTopology::with_valkey());
    let mut left = planner.local_for_testing();
    let mut right = left.clone();

    assert_eq!(left.backend_kind(), CacheBackendKind::Valkey);
    assert!(!left.backend_is_shared());

    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:2",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    left.insert(
        plan.application().unwrap(),
        "<html>shared</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Miss);
}

#[test]
fn no_store_http_policy_can_coexist_with_private_application_cache() {
    let planner = CachePlanner::new(CacheTopology::moka_only());
    let app_policy = ApplicationCachePolicy::new(
        CacheScope::private()
            .with_user("user-123")
            .unwrap()
            .with_session("sess-456")
            .unwrap(),
        FreshnessPolicy::new(Duration::from_secs(30), None).unwrap(),
        InvalidationSet::from_tags([tag("account:dashboard"), tag("user:user-123")]),
    )
    .unwrap();
    let http_policy = HttpCachePolicy::new(
        CacheScope::no_store(),
        None,
        ResponseValidators::default(),
        InvalidationSet::new(),
    )
    .unwrap();

    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("account.dashboard").unwrap(),
                "dashboard",
                http_policy,
            )
            .unwrap()
            .with_application_policy(app_policy),
        )
        .unwrap();

    let application = plan.application().unwrap();
    assert_eq!(application.layers().l2, None);
    assert_eq!(application.coalescing(), RequestCoalescingMode::Local);
    assert_eq!(
        application.key().to_string(),
        "account.dashboard:dashboard|visibility=private|user=user-123|session=sess-456"
    );
    assert_eq!(plan.http().cache_control(), "no-store");
    assert!(!plan.http().edge_cacheable());
    assert_eq!(plan.http().variation(), None);
}

#[test]
fn no_store_http_policy_rejects_freshness_and_cacheable_http_requires_it() {
    assert_eq!(
        HttpCachePolicy::new(
            CacheScope::no_store(),
            Some(FreshnessPolicy::new(Duration::from_secs(10), None).unwrap()),
            ResponseValidators::default(),
            InvalidationSet::new(),
        )
        .unwrap_err(),
        CacheModelError::NoStoreCannotDefineFreshness
    );

    assert_eq!(
        HttpCachePolicy::new(
            CacheScope::private(),
            None,
            ResponseValidators::default(),
            InvalidationSet::new(),
        )
        .unwrap_err(),
        CacheModelError::MissingHttpFreshness
    );
}

#[test]
fn runtime_serves_fresh_then_stale_then_miss() {
    let planner = CachePlanner::new(CacheTopology::with_valkey());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.page").unwrap(),
                "page:42",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(
                        FreshnessPolicy::new(
                            Duration::from_secs(60),
                            Some(Duration::from_secs(30)),
                        )
                        .unwrap(),
                    ),
                    ResponseValidators::default(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(30)))
                        .unwrap(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let application = plan.application().unwrap();
    let mut runtime = planner.runtime();
    runtime.insert(
        application,
        "<html>cached</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let fresh = runtime.lookup(application.key(), CacheInstant::from_unix_seconds(140));
    assert_eq!(fresh.state, CacheLookupState::Fresh);
    assert!(!fresh.needs_revalidation);

    let stale = runtime.lookup(application.key(), CacheInstant::from_unix_seconds(170));
    assert_eq!(stale.state, CacheLookupState::Stale);
    assert!(stale.needs_revalidation);

    let miss = runtime.lookup(application.key(), CacheInstant::from_unix_seconds(195));
    assert_eq!(miss.state, CacheLookupState::Miss);
    assert_eq!(runtime.metrics().hits, 1);
    assert_eq!(runtime.metrics().stale_hits, 1);
    assert_eq!(runtime.metrics().misses, 1);
}

#[test]
fn runtime_keeps_public_and_private_entries_isolated() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let public_plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.page").unwrap(),
                "page:42",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let private_plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.page").unwrap(),
                "page:42",
                HttpCachePolicy::new(
                    CacheScope::private(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::private(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    assert_ne!(
        public_plan.application().unwrap().key(),
        private_plan.application().unwrap().key()
    );

    let mut runtime = planner.runtime();
    runtime.insert(
        public_plan.application().unwrap(),
        "public",
        CacheInstant::from_unix_seconds(100),
    );
    runtime.insert(
        private_plan.application().unwrap(),
        "private",
        CacheInstant::from_unix_seconds(100),
    );

    assert_eq!(
        runtime
            .lookup(
                public_plan.application().unwrap().key(),
                CacheInstant::from_unix_seconds(110)
            )
            .entry
            .as_ref()
            .map(|entry| entry.value.as_str()),
        Some("public")
    );
    assert_eq!(
        runtime
            .lookup(
                private_plan.application().unwrap().key(),
                CacheInstant::from_unix_seconds(110)
            )
            .entry
            .as_ref()
            .map(|entry| entry.value.as_str()),
        Some("private")
    );
}

#[test]
fn runtime_invalidates_entries_by_surrogate_tag() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let page_plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.page").unwrap(),
                "page:42",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::from_tags([tag("page:42")]),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(300), None).unwrap(),
                    InvalidationSet::from_tags([tag("page:42"), tag("nav:main")]),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let nav_plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.nav").unwrap(),
                "nav:main",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::from_tags([tag("nav:main")]),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(300), None).unwrap(),
                    InvalidationSet::from_tags([tag("nav:main")]),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let mut runtime = planner.runtime();
    runtime.insert(
        page_plan.application().unwrap(),
        "page",
        CacheInstant::from_unix_seconds(100),
    );
    runtime.insert(
        nav_plan.application().unwrap(),
        "nav",
        CacheInstant::from_unix_seconds(100),
    );

    let removed = runtime.invalidate(&InvalidationSet::from_tags([tag("nav:main")]));
    assert_eq!(removed.len(), 2);
    assert_eq!(
        runtime
            .lookup(
                page_plan.application().unwrap().key(),
                CacheInstant::from_unix_seconds(110)
            )
            .state,
        CacheLookupState::Miss
    );
    assert_eq!(runtime.metrics().invalidations, 2);
}

#[test]
fn runtime_coalesces_duplicate_fill_requests() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "product:sku-1",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let key = plan.application().unwrap().key().clone();
    let mut runtime = planner.runtime();

    let first = runtime.begin_fill(&key, RequestCoalescingMode::Cluster, "request-a");
    let lease = match first {
        FillDecision::Start(lease) => lease,
        other => panic!("expected fill lease, got {other:?}"),
    };

    let second = runtime.begin_fill(&key, RequestCoalescingMode::Cluster, "request-b");
    assert!(matches!(
        second,
        FillDecision::Coalesced { ref holder, .. } if holder == "request-a"
    ));
    runtime.complete_fill(&lease).unwrap();
    assert_eq!(runtime.metrics().fills_started, 1);
    assert_eq!(runtime.metrics().fills_completed, 1);
    assert_eq!(runtime.metrics().coalesced_waits, 1);
}

#[test]
fn distributed_planner_runtimes_do_not_share_backend_without_explicit_client_wiring() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:shared",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();

    let mut left = planner.local_for_testing();
    let mut right = planner.local_for_testing();
    left.insert(
        plan.application().unwrap(),
        "<html>shared-across-handles</html>",
        CacheInstant::from_unix_seconds(200),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(210),
    );
    assert_eq!(lookup.state, CacheLookupState::Miss);
}

#[test]
fn distributed_runtimes_share_backend_when_reusing_an_explicit_client() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:shared",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let adapter = CacheBackendAdapter::distributed(
        CacheTopology::with_redis(),
        DistributedCacheClient::local_for_testing(CacheBackendKind::Redis),
    );
    let mut left = CacheRuntime::with_backend(CacheTopology::with_redis(), adapter.clone());
    let mut right = CacheRuntime::with_backend(CacheTopology::with_redis(), adapter);

    left.insert(
        plan.application().unwrap(),
        "<html>shared-across-handles</html>",
        CacheInstant::from_unix_seconds(200),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(210),
    );
    assert_eq!(lookup.state, CacheLookupState::Fresh);
}

#[derive(Default)]
struct RecordingDistributedCacheRuntime {
    inserted: Mutex<Vec<String>>,
    looked_up: Mutex<Vec<String>>,
    stored_entry: Mutex<Option<CacheEntry>>,
}

impl DistributedCacheRuntime for RecordingDistributedCacheRuntime {
    fn insert(&self, entry: CacheEntry) {
        self.inserted
            .lock()
            .expect("recording cache insert mutex poisoned")
            .push(entry.key.to_string());
        *self
            .stored_entry
            .lock()
            .expect("recording cache entry mutex poisoned") = Some(entry);
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.looked_up
            .lock()
            .expect("recording cache lookup mutex poisoned")
            .push(key.to_string());
        let entry = self
            .stored_entry
            .lock()
            .expect("recording cache entry mutex poisoned")
            .clone();
        match entry.filter(|entry| &entry.key == key && entry.is_fresh(now)) {
            Some(entry) => CacheLookup {
                state: CacheLookupState::Fresh,
                entry: Some(entry),
                needs_revalidation: false,
            },
            None => CacheLookup {
                state: CacheLookupState::Miss,
                entry: None,
                needs_revalidation: false,
            },
        }
    }

    fn invalidate(&self, _tags: &InvalidationSet) -> Vec<CacheKey> {
        Vec::new()
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        _mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        FillDecision::Start(FillLease {
            key: key.clone(),
            holder,
        })
    }

    fn complete_fill(&self, _lease: &FillLease) -> Result<(), CacheModelError> {
        Ok(())
    }

    fn metrics(&self) -> CacheMetrics {
        CacheMetrics::default()
    }
}

#[test]
fn runtime_accepts_injected_distributed_backend_clients() {
    let planner = CachePlanner::new(CacheTopology::with_redis());
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("catalog.page").unwrap(),
                "page:adapter",
                HttpCachePolicy::new(
                    CacheScope::public(),
                    Some(FreshnessPolicy::new(Duration::from_secs(60), None).unwrap()),
                    ResponseValidators::default(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            )
            .unwrap()
            .with_application_policy(
                ApplicationCachePolicy::new(
                    CacheScope::public(),
                    FreshnessPolicy::new(Duration::from_secs(60), None).unwrap(),
                    InvalidationSet::new(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
    let backend = Arc::new(RecordingDistributedCacheRuntime::default());
    let client = DistributedCacheClient::new(CacheBackendKind::Redis, backend.clone());
    let adapter = CacheBackendAdapter::distributed(CacheTopology::with_redis(), client);
    let mut runtime = CacheRuntime::with_backend(CacheTopology::with_redis(), adapter);

    runtime.insert(
        plan.application().unwrap(),
        "<html>adapter</html>",
        CacheInstant::from_unix_seconds(300),
    );
    let lookup = runtime.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(305),
    );

    assert_eq!(lookup.state, CacheLookupState::Fresh);
    assert_eq!(
        backend
            .inserted
            .lock()
            .expect("recording cache insert mutex poisoned")
            .as_slice(),
        &[plan.application().unwrap().key().to_string()]
    );
    assert_eq!(
        backend
            .looked_up
            .lock()
            .expect("recording cache lookup mutex poisoned")
            .as_slice(),
        &[plan.application().unwrap().key().to_string()]
    );
}
