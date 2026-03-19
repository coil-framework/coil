use crate::{
    ApplicationCachePolicy, CacheKey, CacheModelError, CacheNamespace, CacheScope, CacheTopology,
    DistributedCacheBackend, FreshnessPolicy, HttpCachePolicy, InvalidationSet,
    RequestCoalescingMode, ResponseValidators, VariationKey,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePlanRequest {
    namespace: CacheNamespace,
    resource: String,
    application_policy: Option<ApplicationCachePolicy>,
    http_policy: HttpCachePolicy,
    request_coalescing_mode: Option<RequestCoalescingMode>,
}

impl CachePlanRequest {
    pub fn new(
        namespace: CacheNamespace,
        resource: impl Into<String>,
        http_policy: HttpCachePolicy,
    ) -> Result<Self, CacheModelError> {
        Ok(Self {
            namespace,
            resource: crate::types::require_non_empty("cache_resource", resource.into())?,
            application_policy: None,
            http_policy,
            request_coalescing_mode: None,
        })
    }

    pub fn with_application_policy(mut self, policy: ApplicationCachePolicy) -> Self {
        self.application_policy = Some(policy);
        self
    }

    pub fn with_request_coalescing_mode(mut self, mode: RequestCoalescingMode) -> Self {
        self.request_coalescing_mode = Some(mode);
        self
    }

    pub fn request_coalescing_mode(&self) -> Option<RequestCoalescingMode> {
        self.request_coalescing_mode
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLayerPlan {
    pub l1: crate::LocalCacheBackend,
    pub l2: Option<DistributedCacheBackend>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCachePlan {
    key: CacheKey,
    scope: CacheScope,
    freshness: FreshnessPolicy,
    tags: InvalidationSet,
    layers: CacheLayerPlan,
    coalescing: RequestCoalescingMode,
}

impl ApplicationCachePlan {
    pub fn key(&self) -> &CacheKey {
        &self.key
    }

    pub fn scope(&self) -> &CacheScope {
        &self.scope
    }

    pub fn freshness(&self) -> FreshnessPolicy {
        self.freshness
    }

    pub fn tags(&self) -> &InvalidationSet {
        &self.tags
    }

    pub fn layers(&self) -> &CacheLayerPlan {
        &self.layers
    }

    pub fn coalescing(&self) -> RequestCoalescingMode {
        self.coalescing
    }

    pub fn shared_invalidation(&self) -> bool {
        self.layers.l2.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCachePlan {
    scope: CacheScope,
    variation: Option<VariationKey>,
    validators: ResponseValidators,
    surrogate_tags: InvalidationSet,
    cache_control: String,
}

impl HttpCachePlan {
    pub fn scope(&self) -> &CacheScope {
        &self.scope
    }

    pub fn variation(&self) -> Option<&VariationKey> {
        self.variation.as_ref()
    }

    pub fn validators(&self) -> &ResponseValidators {
        &self.validators
    }

    pub fn surrogate_tags(&self) -> &InvalidationSet {
        &self.surrogate_tags
    }

    pub fn cache_control(&self) -> &str {
        &self.cache_control
    }

    pub fn edge_cacheable(&self) -> bool {
        self.scope.is_edge_cacheable() && self.scope.is_cacheable()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePlan {
    application: Option<ApplicationCachePlan>,
    http: HttpCachePlan,
}

impl CachePlan {
    pub fn application(&self) -> Option<&ApplicationCachePlan> {
        self.application.as_ref()
    }

    pub fn http(&self) -> &HttpCachePlan {
        &self.http
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CachePlanner {
    topology: CacheTopology,
}

impl CachePlanner {
    pub fn new(topology: CacheTopology) -> Self {
        Self { topology }
    }

    pub fn topology(&self) -> CacheTopology {
        self.topology
    }

    pub fn runtime(&self) -> crate::CacheRuntime {
        crate::CacheRuntime::with_backend(
            self.topology,
            crate::CacheBackendAdapter::scoped_shared(self.topology, format!("{:p}", self)),
        )
    }

    #[doc(hidden)]
    pub fn local_for_testing(&self) -> crate::CacheRuntime {
        crate::CacheRuntime::local_for_testing(self.topology)
    }

    #[doc(hidden)]
    pub fn local_runtime(&self) -> crate::CacheRuntime {
        self.local_for_testing()
    }

    pub fn shared_runtime(&self) -> crate::CacheRuntime {
        crate::CacheRuntime::with_backend(
            self.topology,
            crate::CacheBackendAdapter::shared(self.topology),
        )
    }

    pub fn plan(&self, request: CachePlanRequest) -> Result<CachePlan, CacheModelError> {
        let CachePlanRequest {
            namespace,
            resource,
            application_policy,
            http_policy,
            request_coalescing_mode,
        } = request;

        let application = application_policy
            .map(|policy| {
                let variation = policy.scope().cache_partition_key();
                let key = CacheKey::new(namespace.clone(), resource.clone(), variation)?;
                let coalescing =
                    request_coalescing_mode.unwrap_or(self.topology.request_coalescing_mode());

                Ok(ApplicationCachePlan {
                    key,
                    scope: policy.scope().clone(),
                    freshness: policy.freshness(),
                    tags: policy.tags().clone(),
                    layers: CacheLayerPlan {
                        l1: self.topology.l1(),
                        l2: self.topology.l2(),
                    },
                    coalescing,
                })
            })
            .transpose()?;

        let http = HttpCachePlan {
            variation: http_policy.scope().variation_key(),
            scope: http_policy.scope().clone(),
            validators: http_policy.validators().clone(),
            surrogate_tags: http_policy.surrogate_tags().clone(),
            cache_control: http_policy.cache_control_value(),
        };

        Ok(CachePlan { application, http })
    }
}

impl PartialEq for CachePlanner {
    fn eq(&self, other: &Self) -> bool {
        self.topology == other.topology
    }
}

impl Eq for CachePlanner {}
