use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheModelError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    PublicScopeCannotVaryByUser,
    PublicScopeCannotVaryBySession,
    UncacheableApplicationScope,
    MissingHttpFreshness,
    NoStoreCannotDefineFreshness,
    ZeroDuration { field: &'static str },
    UnknownInflightFill { key: String },
}

impl fmt::Display for CacheModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::PublicScopeCannotVaryByUser => {
                f.write_str("public cache scopes cannot vary by user")
            }
            Self::PublicScopeCannotVaryBySession => {
                f.write_str("public cache scopes cannot vary by session")
            }
            Self::UncacheableApplicationScope => {
                f.write_str("application cache policy cannot use a no-store scope")
            }
            Self::MissingHttpFreshness => {
                f.write_str("cacheable HTTP responses must define a freshness policy")
            }
            Self::NoStoreCannotDefineFreshness => {
                f.write_str("no-store HTTP responses cannot define freshness")
            }
            Self::ZeroDuration { field } => write!(f, "`{field}` must be greater than zero"),
            Self::UnknownInflightFill { key } => {
                write!(f, "cache fill for `{key}` is not currently in progress")
            }
        }
    }
}

impl Error for CacheModelError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheVisibility {
    Public,
    Private,
    NoStore,
}

impl fmt::Display for CacheVisibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Public => f.write_str("public"),
            Self::Private => f.write_str("private"),
            Self::NoStore => f.write_str("no_store"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheScope {
    visibility: CacheVisibility,
    tenant: Option<String>,
    site: Option<String>,
    locale: Option<String>,
    user: Option<String>,
    session: Option<String>,
    custom: BTreeMap<String, String>,
}

impl CacheScope {
    pub fn public() -> Self {
        Self::new(CacheVisibility::Public)
    }

    pub fn private() -> Self {
        Self::new(CacheVisibility::Private)
    }

    pub fn no_store() -> Self {
        Self::new(CacheVisibility::NoStore)
    }

    pub fn new(visibility: CacheVisibility) -> Self {
        Self {
            visibility,
            tenant: None,
            site: None,
            locale: None,
            user: None,
            session: None,
            custom: BTreeMap::new(),
        }
    }

    pub fn visibility(&self) -> CacheVisibility {
        self.visibility
    }

    pub fn tenant(&self) -> Option<&str> {
        self.tenant.as_deref()
    }

    pub fn site(&self) -> Option<&str> {
        self.site.as_deref()
    }

    pub fn locale(&self) -> Option<&str> {
        self.locale.as_deref()
    }

    pub fn user(&self) -> Option<&str> {
        self.user.as_deref()
    }

    pub fn session(&self) -> Option<&str> {
        self.session.as_deref()
    }

    pub fn custom(&self) -> &BTreeMap<String, String> {
        &self.custom
    }

    pub fn with_tenant(mut self, tenant: impl Into<String>) -> Result<Self, CacheModelError> {
        self.tenant = Some(validate_token("tenant", tenant.into())?);
        Ok(self)
    }

    pub fn with_site(mut self, site: impl Into<String>) -> Result<Self, CacheModelError> {
        self.site = Some(validate_token("site", site.into())?);
        Ok(self)
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Result<Self, CacheModelError> {
        self.locale = Some(validate_token("locale", locale.into())?);
        Ok(self)
    }

    pub fn with_user(mut self, user: impl Into<String>) -> Result<Self, CacheModelError> {
        if self.visibility == CacheVisibility::Public {
            return Err(CacheModelError::PublicScopeCannotVaryByUser);
        }

        self.user = Some(validate_token("user", user.into())?);
        Ok(self)
    }

    pub fn with_session(mut self, session: impl Into<String>) -> Result<Self, CacheModelError> {
        if self.visibility == CacheVisibility::Public {
            return Err(CacheModelError::PublicScopeCannotVaryBySession);
        }

        self.session = Some(validate_token("session", session.into())?);
        Ok(self)
    }

    pub fn with_custom_variation(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, CacheModelError> {
        let name = validate_token("variation_name", name.into())?;
        let value = validate_token("variation_value", value.into())?;
        self.custom.insert(name, value);
        Ok(self)
    }

    pub fn is_cacheable(&self) -> bool {
        self.visibility != CacheVisibility::NoStore
    }

    pub fn is_edge_cacheable(&self) -> bool {
        self.visibility == CacheVisibility::Public
    }

    pub fn variation_key(&self) -> Option<VariationKey> {
        if self.visibility == CacheVisibility::NoStore {
            return None;
        }

        let mut parts = Vec::new();

        if let Some(tenant) = &self.tenant {
            parts.push(format!("tenant={tenant}"));
        }

        if let Some(site) = &self.site {
            parts.push(format!("site={site}"));
        }

        if let Some(locale) = &self.locale {
            parts.push(format!("locale={locale}"));
        }

        if let Some(user) = &self.user {
            parts.push(format!("user={user}"));
        }

        if let Some(session) = &self.session {
            parts.push(format!("session={session}"));
        }

        for (name, value) in &self.custom {
            parts.push(format!("x:{name}={value}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(VariationKey(parts.join("|")))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VariationKey(String);

impl VariationKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VariationKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InvalidationTag(String);

impl InvalidationTag {
    pub fn new(value: impl Into<String>) -> Result<Self, CacheModelError> {
        Ok(Self(validate_token("invalidation_tag", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InvalidationTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvalidationSet {
    tags: BTreeSet<InvalidationTag>,
}

impl InvalidationSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_tags(tags: impl IntoIterator<Item = InvalidationTag>) -> Self {
        let mut set = Self::new();
        for tag in tags {
            set.insert(tag);
        }
        set
    }

    pub fn insert(&mut self, tag: InvalidationTag) {
        self.tags.insert(tag);
    }

    pub fn len(&self) -> usize {
        self.tags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &InvalidationTag> {
        self.tags.iter()
    }

    pub fn header_value(&self) -> Option<String> {
        if self.tags.is_empty() {
            None
        } else {
            Some(
                self.tags
                    .iter()
                    .map(InvalidationTag::as_str)
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessPolicy {
    ttl: Duration,
    stale_while_revalidate: Option<Duration>,
}

impl FreshnessPolicy {
    pub fn new(
        ttl: Duration,
        stale_while_revalidate: Option<Duration>,
    ) -> Result<Self, CacheModelError> {
        if ttl.is_zero() {
            return Err(CacheModelError::ZeroDuration { field: "ttl" });
        }

        if stale_while_revalidate.is_some_and(|value| value.is_zero()) {
            return Err(CacheModelError::ZeroDuration {
                field: "stale_while_revalidate",
            });
        }

        Ok(Self {
            ttl,
            stale_while_revalidate,
        })
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub fn stale_while_revalidate(&self) -> Option<Duration> {
        self.stale_while_revalidate
    }

    pub fn ttl_seconds(&self) -> u64 {
        self.ttl.as_secs()
    }

    pub fn stale_while_revalidate_seconds(&self) -> Option<u64> {
        self.stale_while_revalidate.map(|value| value.as_secs())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityTag(String);

impl EntityTag {
    pub fn new(value: impl Into<String>) -> Result<Self, CacheModelError> {
        Ok(Self(validate_token("etag", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResponseValidators {
    pub etag: Option<EntityTag>,
    pub last_modified_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationCachePolicy {
    scope: CacheScope,
    freshness: FreshnessPolicy,
    tags: InvalidationSet,
}

impl ApplicationCachePolicy {
    pub fn new(
        scope: CacheScope,
        freshness: FreshnessPolicy,
        tags: InvalidationSet,
    ) -> Result<Self, CacheModelError> {
        if !scope.is_cacheable() {
            return Err(CacheModelError::UncacheableApplicationScope);
        }

        Ok(Self {
            scope,
            freshness,
            tags,
        })
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpCachePolicy {
    scope: CacheScope,
    freshness: Option<FreshnessPolicy>,
    validators: ResponseValidators,
    surrogate_tags: InvalidationSet,
}

impl HttpCachePolicy {
    pub fn new(
        scope: CacheScope,
        freshness: Option<FreshnessPolicy>,
        validators: ResponseValidators,
        surrogate_tags: InvalidationSet,
    ) -> Result<Self, CacheModelError> {
        match (scope.is_cacheable(), freshness) {
            (true, None) => Err(CacheModelError::MissingHttpFreshness),
            (false, Some(_)) => Err(CacheModelError::NoStoreCannotDefineFreshness),
            _ => Ok(Self {
                scope,
                freshness,
                validators,
                surrogate_tags,
            }),
        }
    }

    pub fn scope(&self) -> &CacheScope {
        &self.scope
    }

    pub fn freshness(&self) -> Option<FreshnessPolicy> {
        self.freshness
    }

    pub fn validators(&self) -> &ResponseValidators {
        &self.validators
    }

    pub fn surrogate_tags(&self) -> &InvalidationSet {
        &self.surrogate_tags
    }

    pub fn cache_control_value(&self) -> String {
        match (self.scope.visibility(), self.freshness) {
            (CacheVisibility::NoStore, _) => "no-store".to_string(),
            (CacheVisibility::Public, Some(freshness)) => cache_control_value("public", freshness),
            (CacheVisibility::Private, Some(freshness)) => {
                cache_control_value("private", freshness)
            }
            (_, None) => "no-store".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CacheNamespace(String);

impl CacheNamespace {
    pub fn new(value: impl Into<String>) -> Result<Self, CacheModelError> {
        Ok(Self(validate_token("cache_namespace", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CacheNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CacheKey {
    namespace: CacheNamespace,
    resource: String,
    variation: Option<VariationKey>,
}

impl CacheKey {
    pub fn new(
        namespace: CacheNamespace,
        resource: impl Into<String>,
        variation: Option<VariationKey>,
    ) -> Result<Self, CacheModelError> {
        Ok(Self {
            namespace,
            resource: require_non_empty("cache_resource", resource.into())?,
            variation,
        })
    }

    pub fn namespace(&self) -> &CacheNamespace {
        &self.namespace
    }

    pub fn resource(&self) -> &str {
        &self.resource
    }

    pub fn variation(&self) -> Option<&VariationKey> {
        self.variation.as_ref()
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variation {
            Some(variation) => write!(f, "{}:{}|{}", self.namespace, self.resource, variation),
            None => write!(f, "{}:{}", self.namespace, self.resource),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalCacheBackend {
    Moka,
}

impl fmt::Display for LocalCacheBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Moka => f.write_str("moka"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistributedCacheBackend {
    Redis,
    Valkey,
}

impl fmt::Display for DistributedCacheBackend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Redis => f.write_str("redis"),
            Self::Valkey => f.write_str("valkey"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestCoalescingMode {
    Disabled,
    Local,
    Cluster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheTopology {
    l1: LocalCacheBackend,
    l2: Option<DistributedCacheBackend>,
}

impl CacheTopology {
    pub fn moka_only() -> Self {
        Self {
            l1: LocalCacheBackend::Moka,
            l2: None,
        }
    }

    pub fn with_redis() -> Self {
        Self {
            l1: LocalCacheBackend::Moka,
            l2: Some(DistributedCacheBackend::Redis),
        }
    }

    pub fn with_valkey() -> Self {
        Self {
            l1: LocalCacheBackend::Moka,
            l2: Some(DistributedCacheBackend::Valkey),
        }
    }

    pub fn l1(&self) -> LocalCacheBackend {
        self.l1
    }

    pub fn l2(&self) -> Option<DistributedCacheBackend> {
        self.l2
    }

    pub fn supports_shared_invalidation(&self) -> bool {
        self.l2.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePlanRequest {
    namespace: CacheNamespace,
    resource: String,
    application_policy: Option<ApplicationCachePolicy>,
    http_policy: HttpCachePolicy,
}

impl CachePlanRequest {
    pub fn new(
        namespace: CacheNamespace,
        resource: impl Into<String>,
        http_policy: HttpCachePolicy,
    ) -> Result<Self, CacheModelError> {
        Ok(Self {
            namespace,
            resource: require_non_empty("cache_resource", resource.into())?,
            application_policy: None,
            http_policy,
        })
    }

    pub fn with_application_policy(mut self, policy: ApplicationCachePolicy) -> Self {
        self.application_policy = Some(policy);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLayerPlan {
    pub l1: LocalCacheBackend,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheInstant(u64);

impl CacheInstant {
    pub const fn from_unix_seconds(seconds: u64) -> Self {
        Self(seconds)
    }

    pub const fn as_unix_seconds(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub key: CacheKey,
    pub value: String,
    pub stored_at: CacheInstant,
    pub freshness: FreshnessPolicy,
    pub tags: InvalidationSet,
    pub scope: CacheScope,
    pub layers: CacheLayerPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheLookupState {
    Miss,
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheLookup {
    pub state: CacheLookupState,
    pub entry: Option<CacheEntry>,
    pub needs_revalidation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetrics {
    pub hits: u64,
    pub stale_hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub coalesced_waits: u64,
    pub fills_started: u64,
    pub fills_completed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FillDecision {
    Start(FillLease),
    Coalesced { key: CacheKey, holder: String },
    Uncoalesced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillLease {
    pub key: CacheKey,
    pub holder: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheRuntime {
    topology: CacheTopology,
    entries: BTreeMap<CacheKey, CacheEntry>,
    inflight_fills: BTreeMap<CacheKey, String>,
    metrics: CacheMetrics,
}

impl CacheRuntime {
    pub fn new(topology: CacheTopology) -> Self {
        Self {
            topology,
            entries: BTreeMap::new(),
            inflight_fills: BTreeMap::new(),
            metrics: CacheMetrics::default(),
        }
    }

    pub fn topology(&self) -> CacheTopology {
        self.topology
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.metrics
    }

    pub fn insert(
        &mut self,
        plan: &ApplicationCachePlan,
        value: impl Into<String>,
        now: CacheInstant,
    ) {
        let entry = CacheEntry {
            key: plan.key.clone(),
            value: value.into(),
            stored_at: now,
            freshness: plan.freshness,
            tags: plan.tags.clone(),
            scope: plan.scope.clone(),
            layers: plan.layers.clone(),
        };
        self.entries.insert(plan.key.clone(), entry);
    }

    pub fn lookup(&mut self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        let entry = self.entries.get(key).cloned();
        let Some(entry) = entry else {
            self.metrics.misses += 1;
            return CacheLookup {
                state: CacheLookupState::Miss,
                entry: None,
                needs_revalidation: false,
            };
        };

        let age = now
            .as_unix_seconds()
            .saturating_sub(entry.stored_at.as_unix_seconds());
        if age <= entry.freshness.ttl_seconds() {
            self.metrics.hits += 1;
            return CacheLookup {
                state: CacheLookupState::Fresh,
                entry: Some(entry),
                needs_revalidation: false,
            };
        }

        let max_stale_age = entry
            .freshness
            .stale_while_revalidate_seconds()
            .map(|swr| entry.freshness.ttl_seconds().saturating_add(swr));
        if max_stale_age.is_some_and(|max_age| age <= max_age) {
            self.metrics.stale_hits += 1;
            return CacheLookup {
                state: CacheLookupState::Stale,
                entry: Some(entry),
                needs_revalidation: true,
            };
        }

        self.entries.remove(key);
        self.metrics.misses += 1;
        CacheLookup {
            state: CacheLookupState::Miss,
            entry: None,
            needs_revalidation: false,
        }
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        if tags.is_empty() {
            return Vec::new();
        }

        let removed = self
            .entries
            .iter()
            .filter(|(_, entry)| {
                entry
                    .tags
                    .iter()
                    .any(|tag| tags.iter().any(|wanted| wanted == tag))
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        for key in &removed {
            self.entries.remove(key);
        }
        self.metrics.invalidations += removed.len() as u64;
        removed
    }

    pub fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        let holder = holder.into();
        if mode == RequestCoalescingMode::Disabled {
            return FillDecision::Uncoalesced;
        }

        if let Some(existing) = self.inflight_fills.get(key) {
            self.metrics.coalesced_waits += 1;
            return FillDecision::Coalesced {
                key: key.clone(),
                holder: existing.clone(),
            };
        }

        self.inflight_fills.insert(key.clone(), holder.clone());
        self.metrics.fills_started += 1;
        FillDecision::Start(FillLease {
            key: key.clone(),
            holder,
        })
    }

    pub fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        match self.inflight_fills.remove(&lease.key) {
            Some(_) => {
                self.metrics.fills_completed += 1;
                Ok(())
            }
            None => Err(CacheModelError::UnknownInflightFill {
                key: lease.key.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn runtime(&self) -> CacheRuntime {
        CacheRuntime::new(self.topology)
    }

    pub fn plan(&self, request: CachePlanRequest) -> Result<CachePlan, CacheModelError> {
        let namespace = request.namespace.clone();
        let resource = request.resource.clone();

        let application = request
            .application_policy
            .map(|policy| {
                let variation = policy.scope().variation_key();
                let key = CacheKey::new(namespace.clone(), resource.clone(), variation)?;
                let coalescing = if self.topology.supports_shared_invalidation() {
                    RequestCoalescingMode::Cluster
                } else {
                    RequestCoalescingMode::Local
                };

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
            variation: request.http_policy.scope().variation_key(),
            scope: request.http_policy.scope().clone(),
            validators: request.http_policy.validators().clone(),
            surrogate_tags: request.http_policy.surrogate_tags().clone(),
            cache_control: request.http_policy.cache_control_value(),
        };

        Ok(CachePlan { application, http })
    }
}

fn cache_control_value(visibility: &str, freshness: FreshnessPolicy) -> String {
    let mut directives = vec![format!("{visibility}, max-age={}", freshness.ttl_seconds())];

    if let Some(swr) = freshness.stale_while_revalidate_seconds() {
        directives.push(format!("stale-while-revalidate={swr}"));
    }

    directives.join(", ")
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, CacheModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CacheModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, CacheModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(CacheModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(value: &str) -> InvalidationTag {
        InvalidationTag::new(value).unwrap()
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
            Some(
                FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(15)))
                    .unwrap(),
            ),
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
            "cms.page:page:42|site=main|locale=en-GB"
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
            plan.http().surrogate_tags().header_value().as_deref(),
            Some("jsonld:page:42 page:42")
        );
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
            "account.dashboard:dashboard|user=user-123|session=sess-456"
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
                        FreshnessPolicy::new(
                            Duration::from_secs(60),
                            Some(Duration::from_secs(30)),
                        )
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
}
