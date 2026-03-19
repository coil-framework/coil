use super::*;

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
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
        tenant_id_from_runtime(
            self.config.app.name.as_str(),
            self.config.app.environment,
            self.config.seo.canonical_host.as_str(),
        )
    }

    pub fn jobs_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<JobsHost, RuntimeJobsError> {
        let scheduler_node_id =
            validate_runtime_identifier("scheduler_node_id", scheduler_node_id.into())?;

        Ok(JobsHost::new(
            self.config.app.name.clone(),
            scheduler_node_id,
            self.jobs.clone(),
            self.jobs.describe().clone(),
            self.registered_runtime_jobs.clone(),
            self.registered_runtime_event_subscriptions.clone(),
            self.jobs_domain.clone(),
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
        Ok(CacheHost::new(
            self.config.app.name.clone(),
            namespace,
            self.cache_planner,
        ))
    }

    pub fn browser_host(&self) -> BrowserHost {
        BrowserHost::new(self.config.app.name.clone(), self.browser.clone())
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
        Ok(HttpServerHost::new(
            self.clone(),
            self.shared_backend_clients(resolver)?,
            cookie_secret.to_vec(),
            csrf_secret.to_vec(),
        ))
    }

    pub(crate) fn cache_namespace(&self) -> Result<CacheNamespace, CacheModelError> {
        CacheNamespace::new(format!("customer-app:{}", self.config.app.name))
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

fn tenant_id_from_runtime(
    app_name: &str,
    environment: davenda_config::Environment,
    canonical_host: &str,
) -> i64 {
    let mut hash = 0xcbf29ce484222325u64;
    for value in [
        "davenda-tenant",
        app_name,
        canonical_host,
        environment_label(environment),
    ] {
        for byte in value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }

    let value = hash & i64::MAX as u64;
    if value == 0 {
        1
    } else {
        value as i64
    }
}

fn environment_label(environment: davenda_config::Environment) -> &'static str {
    match environment {
        davenda_config::Environment::Development => "development",
        davenda_config::Environment::Staging => "staging",
        davenda_config::Environment::Production => "production",
    }
}
