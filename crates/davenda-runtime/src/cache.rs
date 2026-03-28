use super::*;
use std::sync::Arc;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeCacheError {
    #[error(transparent)]
    Cache(#[from] CacheModelError),
}

#[derive(Debug, Clone)]
pub struct CacheHost {
    pub customer_app: String,
    pub namespace: CacheNamespace,
    pub shared_backend_namespace: String,
    pub planner: CachePlanner,
    runtime: CacheRuntime,
}

impl CacheHost {
    pub(crate) fn new(
        customer_app: String,
        namespace: CacheNamespace,
        planner: CachePlanner,
        shared_runtime: Option<Arc<dyn davenda_cache::DistributedCacheRuntime>>,
        shared_backend_namespace: String,
    ) -> Self {
        let runtime = match (
            planner.topology().supports_shared_invalidation(),
            shared_runtime,
        ) {
            (true, Some(runtime)) => CacheRuntime::with_shared_runtime(planner.topology(), runtime),
            _ => planner.runtime(),
        };
        Self {
            customer_app,
            namespace,
            planner,
            runtime,
            shared_backend_namespace,
        }
    }

    pub fn lookup_execution(
        &mut self,
        execution: &RequestExecution,
        now: CacheInstant,
    ) -> Option<CacheLookup> {
        execution
            .cache_plan
            .plan
            .application()
            .map(|plan| self.runtime.lookup(plan.key(), now))
    }

    pub fn begin_fill(
        &mut self,
        execution: &RequestExecution,
        holder: impl Into<String>,
    ) -> Option<FillDecision> {
        execution.cache_plan.plan.application().map(|plan| {
            self.runtime
                .begin_fill(plan.key(), plan.coalescing(), holder)
        })
    }

    pub fn complete_fill(&mut self, decision: &FillDecision) -> Result<(), RuntimeCacheError> {
        match decision {
            FillDecision::Start(lease) => Ok(self.runtime.complete_fill(lease)?),
            FillDecision::Coalesced { .. } | FillDecision::Uncoalesced => Ok(()),
        }
    }

    pub fn store_execution(
        &mut self,
        execution: &RequestExecution,
        value: impl Into<String>,
        now: CacheInstant,
    ) -> Option<CacheKey> {
        execution.cache_plan.plan.application().map(|plan| {
            self.runtime.insert(plan, value, now);
            plan.key().clone()
        })
    }

    pub fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.runtime.invalidate(tags)
    }

    pub fn metrics(&self) -> CacheMetrics {
        self.runtime.metrics()
    }
}

pub(crate) fn cache_disposition_for_route(
    method: HttpMethod,
    auth: &RouteAuthGate,
    session: &SessionContext,
) -> CacheDisposition {
    if method.is_state_changing() {
        return CacheDisposition::Uncacheable;
    }

    match auth {
        RouteAuthGate::Public if session.session_id.is_none() => CacheDisposition::Public,
        _ => CacheDisposition::Private,
    }
}

pub(crate) fn build_execution_cache_plan(
    runtime: &RuntimePlan,
    request: &RequestInput,
    route: &RouteDefinition,
    resolved: &ResolvedRoute,
    session: &SessionContext,
    principal: &PrincipalContext,
    disposition: CacheDisposition,
) -> Result<ExecutedCachePlan, CacheModelError> {
    let scope = cache_scope_for_request(request, resolved, session, principal, disposition)?;
    let tags = cache_tags_for_request(runtime, route, resolved, request)?;
    let validators = cache_validators_for_request(request, resolved, session, principal)?;
    let freshness = cache_freshness_for_request(route, request.method, disposition);
    let http_policy = HttpCachePolicy::new(scope.clone(), freshness, validators, tags.clone())?;
    let mut cache_request = CachePlanRequest::new(
        runtime.cache_namespace()?,
        request.path.clone(),
        http_policy,
    )?;

    if let Some(freshness) = freshness.filter(|_| disposition != CacheDisposition::Uncacheable) {
        cache_request = cache_request
            .with_application_policy(ApplicationCachePolicy::new(scope, freshness, tags)?);
    }

    let plan = runtime.cache_planner.plan(cache_request)?;
    let headers = cache_headers_from_plan(&plan);

    Ok(ExecutedCachePlan { plan, headers })
}

fn cache_scope_for_request(
    request: &RequestInput,
    resolved: &ResolvedRoute,
    session: &SessionContext,
    principal: &PrincipalContext,
    disposition: CacheDisposition,
) -> Result<CacheScope, CacheModelError> {
    let mut scope = match disposition {
        CacheDisposition::Public => CacheScope::public(),
        CacheDisposition::Private => CacheScope::private(),
        CacheDisposition::Uncacheable => CacheScope::no_store(),
    }
    .with_site(
        resolved
            .site_id
            .clone()
            .unwrap_or_else(|| request.host.clone()),
    )?;

    if let Some(locale) = resolved.locale.as_deref() {
        scope = scope.with_locale(locale.to_string())?;
    }

    if disposition == CacheDisposition::Private {
        if let Some(principal_id) = principal.principal_id.as_deref() {
            scope = scope.with_user(principal_id.to_string())?;
        } else if let Some(session_id) = session.session_id.as_deref() {
            scope = scope.with_session(session_id.to_string())?;
        }
    }

    Ok(scope)
}

fn cache_tags_for_request(
    runtime: &RuntimePlan,
    route: &RouteDefinition,
    resolved: &ResolvedRoute,
    request: &RequestInput,
) -> Result<InvalidationSet, CacheModelError> {
    let mut tags = InvalidationSet::new();
    tags.insert(InvalidationTag::new(format!(
        "customer_app:{}",
        runtime.config.app.name
    ))?);
    if let Some(site_id) = resolved.site_id.as_deref() {
        tags.insert(InvalidationTag::new(format!("site:{site_id}"))?);
    }
    tags.insert(InvalidationTag::new(format!(
        "route:{}",
        resolved.route_name
    ))?);
    tags.insert(InvalidationTag::new(format!("path:{}", request.path))?);

    if let Some(module) = route.module.as_deref() {
        tags.insert(InvalidationTag::new(format!("module:{module}"))?);
    }

    if let Some(locale) = resolved.locale.as_deref() {
        tags.insert(InvalidationTag::new(format!("locale:{locale}"))?);
    }

    Ok(tags)
}

fn cache_validators_for_request(
    request: &RequestInput,
    resolved: &ResolvedRoute,
    session: &SessionContext,
    principal: &PrincipalContext,
) -> Result<ResponseValidators, CacheModelError> {
    let mut parts = vec![
        "etag".to_string(),
        resolved.route_name.clone(),
        resolved
            .site_id
            .clone()
            .unwrap_or_else(|| request.host.clone()),
        request.path.clone(),
    ];

    if let Some(locale) = resolved.locale.as_deref() {
        parts.push(format!("locale:{locale}"));
    }
    if let Some(principal_id) = principal.principal_id.as_deref() {
        parts.push(format!("user:{principal_id}"));
    } else if let Some(session_id) = session.session_id.as_deref() {
        parts.push(format!("session:{session_id}"));
    }

    Ok(ResponseValidators {
        etag: Some(EntityTag::new(parts.join(":"))?),
        last_modified_unix_seconds: None,
    })
}

fn cache_freshness_for_request(
    route: &RouteDefinition,
    method: HttpMethod,
    disposition: CacheDisposition,
) -> Option<FreshnessPolicy> {
    if method.is_state_changing() || disposition == CacheDisposition::Uncacheable {
        return None;
    }

    match disposition {
        CacheDisposition::Public => Some(
            FreshnessPolicy::new(Duration::from_secs(300), Some(Duration::from_secs(30)))
                .expect("constant public freshness is valid"),
        ),
        CacheDisposition::Private if route.area == RouteArea::Account => Some(
            FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(30)))
                .expect("constant account freshness is valid"),
        ),
        CacheDisposition::Private => Some(
            FreshnessPolicy::new(Duration::from_secs(30), Some(Duration::from_secs(15)))
                .expect("constant private freshness is valid"),
        ),
        CacheDisposition::Uncacheable => None,
    }
}

fn cache_headers_from_plan(plan: &CachePlan) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    headers.insert(
        "Cache-Control".to_string(),
        plan.http().cache_control().to_string(),
    );

    if let Some(variation) = plan.http().variation() {
        headers.insert(
            "X-Davenda-Variation-Key".to_string(),
            variation.as_str().to_string(),
        );
    }

    if let Some(etag) = plan.http().validators().etag.as_ref() {
        headers.insert("ETag".to_string(), etag.as_str().to_string());
    }

    if let Some(surrogate_tags) = plan.http().surrogate_tags().header_value() {
        headers.insert("Surrogate-Key".to_string(), surrogate_tags);
    }

    headers
}
