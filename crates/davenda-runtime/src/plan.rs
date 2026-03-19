use super::*;

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub shared_backend_scope: String,
    pub cache_topology: CacheTopology,
    pub cache_planner: CachePlanner,
    pub i18n: I18nRuntimeServices,
    pub seo: SeoRuntimeServices,
    pub browser: BrowserSecurityServices,
    pub cli: CliRuntimeServices,
    pub data: DataRuntimeServices,
    pub jobs: JobsRuntimeServices,
    pub observability: ObservabilityRuntimeServices,
    pub http: HttpRuntimePlan,
    pub handlers: BTreeMap<String, HandlerDefinition>,
    pub storage_planner: StoragePlanner,
    pub template: TemplateRuntimeServices,
    pub tls: TlsRuntimeServices,
    pub wasm: WasmRuntimeServices,
    pub services: Vec<ServiceDescriptor>,
    pub modules: Vec<ModuleManifest>,
    pub install_migrations: MigrationPlan,
    pub extension_registry: ExtensionRegistry,
    pub registered_extension_slots: Vec<RegisteredExtensionSlot>,
    pub installed_extensions: Vec<InstalledExtensionSummary>,
    pub module_jobs: Vec<RegisteredModuleJob>,
    pub module_event_subscriptions: Vec<RegisteredEventSubscription>,
    pub module_data_repositories: Vec<RegisteredDataRepository>,
    pub module_search_contributions: Vec<RegisteredSearchContribution>,
    pub module_report_definitions: Vec<RegisteredReportDefinition>,
    pub module_bulk_operations: Vec<RegisteredBulkOperation>,
    pub registered_runtime_jobs: Vec<RuntimeJobDefinition>,
    pub registered_runtime_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
    pub jobs_domain: JobsDomain,
    pub ops_catalog: OpsCatalog,
}

impl RuntimePlan {
    pub fn tenant_id(&self) -> i64 {
        self.config.auth.tenant_id
    }

    pub fn jobs_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<JobsHost, RuntimeJobsError> {
        let scheduler_node_id =
            validate_runtime_identifier("scheduler_node_id", scheduler_node_id.into())?;
        let namespace = self.shared_backend_namespace();
        #[cfg(test)]
        let shared_runtime = shared_jobs_runtime_for_test(&self.jobs, namespace.clone());
        #[cfg(not(test))]
        let shared_runtime = davenda_jobs::JobsBackendAdapter::persistent_shared_runtime(
            &self.jobs,
            namespace.clone(),
        );

        Ok(JobsHost::new(
            self.config.app.name.clone(),
            scheduler_node_id,
            self.jobs.clone(),
            self.jobs.describe().clone(),
            self.registered_runtime_jobs.clone(),
            self.registered_runtime_event_subscriptions.clone(),
            self.jobs_domain.clone(),
            shared_runtime,
            namespace,
        ))
    }

    pub fn ops_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<OpsHost, RuntimeOpsError> {
        Ok(OpsHost::new(
            OpsPlanner::new(self.jobs.clone(), self.ops_catalog.clone())?,
            self.jobs_host(scheduler_node_id)?,
        ))
    }

    pub fn search_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<SearchHost, RuntimeSearchError> {
        Ok(SearchHost::new(
            self.ops_catalog.search.clone(),
            self.ops_host(scheduler_node_id)?,
        ))
    }

    pub fn cache_host(&self) -> Result<CacheHost, RuntimeCacheError> {
        let namespace = self.cache_namespace()?;
        let shared_namespace = self.shared_backend_namespace();
        let shared_runtime = if self.cache_planner.topology().supports_shared_invalidation() {
            let backend = match self
                .cache_planner
                .topology()
                .l2()
                .expect("shared cache runtime requires distributed l2")
            {
                davenda_cache::DistributedCacheBackend::Redis => {
                    davenda_cache::CacheBackendKind::Redis
                }
                davenda_cache::DistributedCacheBackend::Valkey => {
                    davenda_cache::CacheBackendKind::Valkey
                }
            };
            #[cfg(test)]
            let runtime = shared_cache_runtime_for_test(backend, shared_namespace.clone());
            #[cfg(not(test))]
            let runtime = davenda_cache::DistributedCacheClient::persistent_shared_runtime(
                backend,
                shared_namespace.clone(),
            );
            Some(runtime)
        } else {
            None
        };
        Ok(CacheHost::new(
            self.config.app.name.clone(),
            namespace,
            self.cache_planner,
            shared_runtime,
            shared_namespace,
        ))
    }

    #[cfg(test)]
    pub fn browser_host(&self) -> Result<BrowserHost, BrowserHostBuildError> {
        BrowserHost::new_with_scope(
            self.config.app.name.clone(),
            self.browser.clone(),
            self.shared_backend_scope.clone(),
        )
    }

    pub fn tls_host(&self) -> TlsHost {
        TlsHost::new(self.config.app.name.clone(), self.tls.clone())
    }

    pub fn storage_host(&self) -> StorageHost {
        StorageHost::new(
            self.config.app.name.clone(),
            self.storage_planner.clone(),
            self.config.assets.cdn_base_url.clone(),
        )
    }

    pub fn wasm_host(&self) -> WasmHost {
        WasmHost::new(
            self.clone(),
            self.config.app.name.clone(),
            self.wasm.clone(),
            self.extension_registry.clone(),
            self.config.i18n.default_locale.clone(),
            self.registered_runtime_jobs.clone(),
        )
    }

    pub fn shared_backend_clients<R: SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<SharedBackendClients, RuntimeServerError> {
        Ok(SharedBackendClients::from_config(&self.config, resolver)?)
    }

    pub fn server_host<R: SecretResolver>(
        &self,
        resolver: &R,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<HttpServerHost, RuntimeServerError> {
        if self.browser.sessions.store == davenda_core::SessionStoreTopology::Memory {
            return Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost.into());
        }

        HttpServerHost::new(
            self.clone(),
            self.shared_backend_clients(resolver)?,
            cookie_secret.to_vec(),
            csrf_secret.to_vec(),
        )
    }

    pub(crate) fn cache_namespace(&self) -> Result<CacheNamespace, CacheModelError> {
        CacheNamespace::new(format!("customer-app:{}", self.config.app.name))
    }

    pub(crate) fn shared_backend_namespace(&self) -> String {
        format!(
            "customer-app:{}:{}",
            self.config.app.name, self.shared_backend_scope
        )
    }

    pub fn execute_request(
        &self,
        request: RequestInput,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<RequestExecution, RequestExecutionError> {
        let matched = self
            .http
            .resolve_match(&self.config, request.method, &request.host, &request.path)
            .ok_or_else(|| RequestExecutionError::RouteNotFound {
                method: request.method,
                host: request.host.clone(),
                path: request.path.clone(),
            })?;

        let trace = RequestTraceContext {
            request_id: request.request_id.clone().unwrap_or_else(|| {
                format!(
                    "req:{:?}:{}:{}",
                    request.method, request.host, matched.resolved.route_name
                )
            }),
            transport_scheme: request
                .forwarded_proto
                .clone()
                .unwrap_or_else(|| request.scheme.clone()),
        };

        let mut resolved_from_cookie = false;
        let session_id = if let Some(session_id) = request.session_id.clone() {
            Some(session_id)
        } else if let Some(cookie) = request.session_cookie.as_deref() {
            resolved_from_cookie = true;
            Some(self.verify_session_cookie(cookie_secret, cookie)?)
        } else {
            None
        };

        let session = SessionContext {
            session_id: session_id.clone(),
            resolved_from_cookie,
        };
        let principal = PrincipalContext {
            principal_id: request.principal_id.clone(),
            granted_capabilities: request.granted_capabilities.clone(),
        };

        self.enforce_maintenance_mode(&matched.route, request.method, &request)?;
        self.enforce_feature_flags(&matched.route)?;
        self.enforce_route_auth(&matched.resolved, &session, &principal)?;
        self.enforce_browser_policy(
            &matched.route,
            &matched.resolved,
            request.method,
            request.csrf_action.as_deref(),
            request.csrf_token.as_deref(),
            &session,
            csrf_secret,
        )?;
        let response = self
            .handlers
            .get(&matched.resolved.route_name)
            .cloned()
            .map(|handler| handler.response)
            .ok_or_else(|| RequestExecutionError::HandlerNotRegistered {
                route: matched.resolved.route_name.clone(),
            })?;
        let cache = cache_disposition_for_route(request.method, &matched.resolved.auth, &session);
        let cache_plan = build_execution_cache_plan(
            self,
            &request,
            &matched.route,
            &matched.resolved,
            &session,
            &principal,
            cache,
        )?;

        Ok(RequestExecution {
            customer_app: self.config.app.name.clone(),
            method: request.method,
            host: request.host,
            path: request.path,
            route: matched.resolved.clone(),
            route_area: matched.route.area,
            locale: matched
                .resolved
                .locale
                .clone()
                .unwrap_or_else(|| self.config.i18n.default_locale.clone()),
            trace,
            session: session.clone(),
            principal,
            cache,
            cache_plan,
            middleware: self.http.middleware.clone(),
            response,
            flash_messages: Vec::new(),
            response_cookies: Vec::new(),
        })
    }

    pub fn execute_browser_request(
        &self,
        browser: &mut BrowserHost,
        mut request: RequestInput,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
        now: BrowserInstant,
    ) -> Result<RequestExecution, RequestExecutionError> {
        let resolved = browser
            .resolve_request(&request, cookie_secret, now)
            .map_err(RequestExecutionError::from_browser_error)?;

        request.session_id = resolved.session.session_id.clone();
        request.session_cookie = None;
        request.flash_cookie = None;

        if request.principal_id.is_none() {
            request.principal_id = resolved.principal_id.clone();
        }

        let mut execution = self.execute_request(request, cookie_secret, csrf_secret)?;
        execution.session = resolved.session;
        if execution.principal.principal_id.is_none() {
            execution.principal.principal_id = resolved.principal_id;
        }
        execution.flash_messages = resolved.flash_messages;
        execution.response_cookies = resolved.response_cookies;
        Ok(execution)
    }

    fn verify_session_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<String, RequestExecutionError> {
        self.browser
            .sessions
            .session_cookie
            .unprotect(cookie_secret, cookie)
            .map_err(|error| RequestExecutionError::InvalidSessionCookie(error.to_string()))
    }

    fn enforce_route_auth(
        &self,
        route: &ResolvedRoute,
        session: &SessionContext,
        principal: &PrincipalContext,
    ) -> Result<(), RequestExecutionError> {
        match route.auth {
            RouteAuthGate::Public => Ok(()),
            RouteAuthGate::Session => {
                if session.session_id.is_some() {
                    Ok(())
                } else {
                    Err(RequestExecutionError::SessionRequired {
                        route: route.route_name.clone(),
                    })
                }
            }
            RouteAuthGate::Capability(capability) => {
                if session.session_id.is_none() {
                    return Err(RequestExecutionError::SessionRequired {
                        route: route.route_name.clone(),
                    });
                }

                if principal.granted_capabilities.contains(&capability) {
                    Ok(())
                } else {
                    Err(RequestExecutionError::CapabilityRequired {
                        route: route.route_name.clone(),
                        capability,
                    })
                }
            }
        }
    }

    fn enforce_feature_flags(&self, route: &RouteDefinition) -> Result<(), RequestExecutionError> {
        let Some(feature_flag) = route.feature_flag.as_deref() else {
            return Ok(());
        };

        let Some(feature_flag_id) = FeatureFlagId::new(feature_flag.to_string()).ok() else {
            return Err(RequestExecutionError::FeatureFlagDisabled {
                route: route.name.clone(),
                feature_flag: feature_flag.to_string(),
            });
        };
        let context = FeatureFlagContext {
            environment: self.config.app.environment,
            customer_app: CustomerAppId::new(self.config.app.name.clone()).ok(),
            site: None,
            brand: None,
            cohorts: BTreeSet::new(),
        };

        match self.observability.flags.get(&feature_flag_id) {
            Some(flag) if flag.enabled_for(&context) => Ok(()),
            _ => Err(RequestExecutionError::FeatureFlagDisabled {
                route: route.name.clone(),
                feature_flag: feature_flag.to_string(),
            }),
        }
    }

    fn enforce_maintenance_mode(
        &self,
        route: &RouteDefinition,
        method: HttpMethod,
        request: &RequestInput,
    ) -> Result<(), RequestExecutionError> {
        let customer_app = CustomerAppId::new(self.config.app.name.clone()).ok();
        let blocked = self.observability.maintenance.blocks_request(
            customer_app.as_ref(),
            method.is_state_changing(),
            request.maintenance_bypass_token.as_deref(),
        );

        if blocked {
            Err(RequestExecutionError::MaintenanceMode {
                route: route.name.clone(),
            })
        } else {
            Ok(())
        }
    }

    fn enforce_browser_policy(
        &self,
        route: &RouteDefinition,
        resolved: &ResolvedRoute,
        method: HttpMethod,
        csrf_action: Option<&str>,
        csrf_token: Option<&str>,
        session: &SessionContext,
        csrf_secret: &[u8],
    ) -> Result<(), RequestExecutionError> {
        let requires_csrf = method.is_state_changing()
            && route.area != RouteArea::Api
            && self.browser.csrf.enabled
            && !matches!(resolved.auth, RouteAuthGate::Public);

        if !requires_csrf {
            return Ok(());
        }

        let session_id = session.session_id.as_deref().ok_or_else(|| {
            RequestExecutionError::MissingSessionForCsrf {
                route: resolved.route_name.clone(),
            }
        })?;
        let token = csrf_token.ok_or_else(|| RequestExecutionError::MissingCsrfToken {
            route: resolved.route_name.clone(),
        })?;
        let action = csrf_action.unwrap_or(&resolved.route_name);
        let valid = self
            .browser
            .csrf
            .verify_token(csrf_secret, session_id, action, token)
            .map_err(|_| RequestExecutionError::InvalidCsrfToken {
                route: resolved.route_name.clone(),
            })?;

        if valid {
            Ok(())
        } else {
            Err(RequestExecutionError::InvalidCsrfToken {
                route: resolved.route_name.clone(),
            })
        }
    }
}

#[cfg(test)]
pub(crate) fn shared_cache_runtime_for_test(
    backend: davenda_cache::CacheBackendKind,
    namespace: String,
) -> std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, Arc<dyn davenda_cache::DistributedCacheRuntime>>>,
    > = OnceLock::new();

    let key = format!("{backend:?}:{namespace}");
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("test cache registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedCacheRuntimeHarness::new(
                davenda_cache::DistributedCacheClient::emulated_shared_runtime(backend),
            ))
        })
        .clone()
}

#[cfg(test)]
pub(crate) fn shared_jobs_runtime_for_test(
    runtime: &JobsRuntimeServices,
    namespace: String,
) -> std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime> {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex, OnceLock};

    static REGISTRY: OnceLock<
        Mutex<BTreeMap<String, Arc<dyn davenda_jobs::JobsCoordinationRuntime>>>,
    > = OnceLock::new();

    let key = format!(
        "{:?}:{}:{}:{}:{}:{}",
        runtime.backend,
        runtime.topology.work_queue.as_str(),
        runtime.topology.scheduled_queue.as_str(),
        runtime.topology.domain_events_queue.as_str(),
        runtime.topology.dead_letter_queue.as_str(),
        namespace
    );
    let registry = REGISTRY.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = registry.lock().expect("test jobs registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedJobsRuntimeHarness::new(
                davenda_jobs::JobsBackendAdapter::emulated_shared_runtime(runtime),
            ))
        })
        .clone()
}

#[cfg(test)]
#[derive(Clone)]
struct SharedCacheRuntimeHarness {
    runtime: std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime>,
}

#[cfg(test)]
impl SharedCacheRuntimeHarness {
    fn new(runtime: std::sync::Arc<dyn davenda_cache::DistributedCacheRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl davenda_cache::DistributedCacheRuntime for SharedCacheRuntimeHarness {
    fn insert(&self, entry: davenda_cache::CacheEntry) {
        self.runtime.insert(entry);
    }

    fn lookup(
        &self,
        key: &davenda_cache::CacheKey,
        now: davenda_cache::CacheInstant,
    ) -> davenda_cache::CacheLookup {
        self.runtime.lookup(key, now)
    }

    fn invalidate(&self, tags: &davenda_cache::InvalidationSet) -> Vec<davenda_cache::CacheKey> {
        self.runtime.invalidate(tags)
    }

    fn begin_fill(
        &self,
        key: &davenda_cache::CacheKey,
        mode: davenda_cache::RequestCoalescingMode,
        holder: String,
    ) -> davenda_cache::FillDecision {
        self.runtime.begin_fill(key, mode, holder)
    }

    fn complete_fill(
        &self,
        lease: &davenda_cache::FillLease,
    ) -> Result<(), davenda_cache::CacheModelError> {
        self.runtime.complete_fill(lease)
    }

    fn metrics(&self) -> davenda_cache::CacheMetrics {
        self.runtime.metrics()
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[cfg(test)]
#[derive(Clone)]
struct SharedJobsRuntimeHarness {
    runtime: std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime>,
}

#[cfg(test)]
impl SharedJobsRuntimeHarness {
    fn new(runtime: std::sync::Arc<dyn davenda_jobs::JobsCoordinationRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl davenda_jobs::JobsCoordinationRuntime for SharedJobsRuntimeHarness {
    fn snapshot(&self) -> davenda_jobs::JobsCoordinatorSnapshot {
        self.runtime.snapshot()
    }

    fn enqueue(
        &self,
        spec: davenda_jobs::JobSpec,
        now: davenda_jobs::JobInstant,
    ) -> Result<(), davenda_jobs::JobsModelError> {
        self.runtime.enqueue(spec, now)
    }

    fn acquire_scheduler_leadership(
        &self,
        node_id: String,
        now: davenda_jobs::JobInstant,
        lease_ttl: std::time::Duration,
    ) -> Result<davenda_jobs::SchedulerLeadership, davenda_jobs::JobsModelError> {
        self.runtime
            .acquire_scheduler_leadership(node_id, now, lease_ttl)
    }

    fn promote_due_jobs(
        &self,
        node_id: &str,
        now: davenda_jobs::JobInstant,
    ) -> Result<Vec<davenda_jobs::JobId>, davenda_jobs::JobsModelError> {
        self.runtime.promote_due_jobs(node_id, now)
    }

    fn lease_ready_jobs(
        &self,
        queue: &davenda_jobs::JobQueueName,
        worker_id: String,
        now: davenda_jobs::JobInstant,
        lease_ttl: std::time::Duration,
        max_jobs: usize,
    ) -> Result<Vec<davenda_jobs::JobLease>, davenda_jobs::JobsModelError> {
        self.runtime
            .lease_ready_jobs(queue, worker_id, now, lease_ttl, max_jobs)
    }

    fn acknowledge_completed(
        &self,
        lease: &davenda_jobs::JobLease,
        now: davenda_jobs::JobInstant,
    ) -> Result<(), davenda_jobs::JobsModelError> {
        self.runtime.acknowledge_completed(lease, now)
    }

    fn acknowledge_failed(
        &self,
        lease: &davenda_jobs::JobLease,
        now: davenda_jobs::JobInstant,
        reason: davenda_jobs::DeadLetterReason,
        error_message: String,
    ) -> Result<davenda_jobs::JobFailureDisposition, davenda_jobs::JobsModelError> {
        self.runtime
            .acknowledge_failed(lease, now, reason, error_message)
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}
