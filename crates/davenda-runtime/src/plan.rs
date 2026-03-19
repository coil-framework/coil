use super::*;
use davenda_template::{
    AttributeNode, DocumentRenderRequest, ElementNode, FragmentRenderRequest, Node, RenderModel,
    RenderValue, TemplateDefinition, TemplateKind, TemplateModelError, TemplateName,
    TemplateNamespace, TemplateRuntime, TemplateSelector, TrustedHtml,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeRenderError {
    #[error(transparent)]
    Template(#[from] TemplateModelError),
}

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub cache_topology: CacheTopology,
    pub cache_planner: CachePlanner,
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

    pub fn render_page_response(
        &self,
        execution: &RequestExecution,
        page: &PageResponse,
    ) -> Result<String, RuntimeRenderError> {
        let selector = template_selector(&page.template)?;
        let namespaces = self.template_namespaces_for_execution(execution);
        let model = self.render_model_for_execution(execution, &page.template, None)?;

        match self.template.runtime.render_document(
            &namespaces,
            DocumentRenderRequest::new(selector.clone(), model.clone()),
        ) {
            Ok(output) => Ok(output.html),
            Err(TemplateModelError::TemplateNotFound { .. })
            | Err(TemplateModelError::TemplateKindMismatch {
                actual: TemplateKind::Fragment,
                ..
            }) => {
                let content =
                    self.render_fragment_content(execution, &namespaces, &selector, model, None)?;
                Ok(self.render_document_shell(execution, &page.template, content)?)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn render_fragment_response(
        &self,
        execution: &RequestExecution,
        fragment: &FragmentResponse,
    ) -> Result<String, RuntimeRenderError> {
        let selector = template_selector(&fragment.template)?;
        let namespaces = self.template_namespaces_for_execution(execution);
        let model = self.render_model_for_execution(
            execution,
            &fragment.template,
            Some(fragment.fragment_id.as_str()),
        )?;

        self.render_fragment_content(
            execution,
            &namespaces,
            &selector,
            model,
            Some(fragment.fragment_id.as_str()),
        )
        .map_err(Into::into)
    }

    fn render_fragment_content(
        &self,
        execution: &RequestExecution,
        namespaces: &[TemplateNamespace],
        selector: &TemplateSelector,
        model: RenderModel,
        fragment_id: Option<&str>,
    ) -> Result<String, TemplateModelError> {
        match self.template.runtime.render_fragment(
            namespaces,
            FragmentRenderRequest::new(selector.clone(), model.clone()),
        ) {
            Ok(output) => Ok(output.html),
            Err(TemplateModelError::TemplateNotFound { .. }) => {
                let runtime = self.synthetic_template_runtime(execution, selector.name(), false)?;
                Ok(runtime
                    .render_fragment(
                        namespaces,
                        FragmentRenderRequest::new(selector.clone(), model),
                    )?
                    .html)
            }
            Err(error) => {
                if matches!(
                    error,
                    TemplateModelError::TemplateKindMismatch {
                        actual: TemplateKind::Layout,
                        ..
                    } | TemplateModelError::FragmentCannotRenderLayout { .. }
                ) && fragment_id.is_none()
                {
                    return Ok(self.render_document_shell(
                        execution,
                        selector.name().as_str(),
                        self.template
                            .runtime
                            .render_document(
                                namespaces,
                                DocumentRenderRequest::new(selector.clone(), model),
                            )?
                            .html,
                    )?);
                }

                Err(error)
            }
        }
    }

    fn render_document_shell(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        content: String,
    ) -> Result<String, TemplateModelError> {
        let shell_name = TemplateName::new("runtime.page.shell")?;
        let shell_selector = TemplateSelector::new(shell_name.clone());
        let mut registry = self.template.registry.clone();
        match registry.register(runtime_page_shell_template(
            self.template.customer_app_namespace.clone(),
        )?) {
            Ok(()) | Err(TemplateModelError::DuplicateTemplate { .. }) => {}
            Err(error) => return Err(error),
        }

        let mut model = self.render_model_for_execution(execution, template_name, None)?;
        model = model
            .with_value(
                "page_title",
                RenderValue::text(format!(
                    "{} · {}",
                    execution.route.route_name, execution.customer_app
                )),
            )?
            .with_value(
                "page_content",
                RenderValue::trusted_html(TrustedHtml::new(content)?),
            )?;

        Ok(TemplateRuntime::new(registry)
            .render_document(
                &[self.template.customer_app_namespace.clone()],
                DocumentRenderRequest::new(shell_selector, model),
            )?
            .html)
    }

    fn synthetic_template_runtime(
        &self,
        execution: &RequestExecution,
        template_name: &TemplateName,
        page_layout: bool,
    ) -> Result<TemplateRuntime, TemplateModelError> {
        let mut registry = self.template.registry.clone();
        let namespace = self
            .module_template_namespace(execution)
            .unwrap_or_else(|| self.template.customer_app_namespace.clone());

        let definition = if page_layout {
            runtime_fallback_page_template(namespace, template_name.clone())?
        } else {
            runtime_fallback_fragment_template(namespace, template_name.clone())?
        };

        registry.register(definition)?;
        Ok(TemplateRuntime::new(registry))
    }

    fn template_namespaces_for_execution(
        &self,
        execution: &RequestExecution,
    ) -> Vec<TemplateNamespace> {
        let module_namespace = self.module_template_namespace(execution);
        self.template.namespace_chain(module_namespace.as_ref())
    }

    fn module_template_namespace(&self, execution: &RequestExecution) -> Option<TemplateNamespace> {
        self.http
            .routes
            .iter()
            .find(|route| route.name == execution.route.route_name)
            .and_then(|route| route.module.as_deref())
            .and_then(|module| TemplateNamespace::new(module.to_string()).ok())
    }

    fn render_model_for_execution(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        fragment_id: Option<&str>,
    ) -> Result<RenderModel, TemplateModelError> {
        let mut model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(execution.customer_app.clone()),
            )?
            .with_value(
                "route_name",
                RenderValue::text(execution.route.route_name.clone()),
            )?
            .with_value("path", RenderValue::text(execution.path.clone()))?
            .with_value("locale", RenderValue::text(execution.locale.clone()))?
            .with_value(
                "method",
                RenderValue::text(format!("{:?}", execution.method)),
            )?
            .with_value(
                "template_name",
                RenderValue::text(template_name.to_string()),
            )?
            .with_value(
                "route_area",
                RenderValue::text(format!("{:?}", execution.route_area)),
            )?
            .with_value(
                "request_id",
                RenderValue::text(execution.trace.request_id.clone()),
            )?
            .with_value(
                "transport_scheme",
                RenderValue::text(execution.trace.transport_scheme.clone()),
            )?
            .with_value(
                "principal_id",
                RenderValue::text(
                    execution
                        .principal
                        .principal_id
                        .clone()
                        .unwrap_or_else(|| "anonymous".to_string()),
                ),
            )?
            .with_value(
                "session_id",
                RenderValue::text(
                    execution
                        .session
                        .session_id
                        .clone()
                        .unwrap_or_else(|| "guest".to_string()),
                ),
            )?
            .with_value(
                "surface_id",
                RenderValue::text(
                    fragment_id
                        .map(str::to_string)
                        .unwrap_or_else(|| execution.route.route_name.clone()),
                ),
            )?;

        if let Some(fragment_id) = fragment_id {
            model = model.with_value("fragment_id", RenderValue::text(fragment_id.to_string()))?;
        }

        Ok(model)
    }

    fn verify_session_cookie(
        &self,
        cookie_secret: &[u8],
        cookie: &str,
    ) -> Result<String, RequestExecutionError> {
        let signer = CookieSigner::new(self.browser.sessions.session_cookie.clone());
        signer
            .verify(cookie_secret, cookie)
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

fn template_selector(template: &str) -> Result<TemplateSelector, TemplateModelError> {
    Ok(TemplateSelector::new(TemplateName::new(
        template.to_string(),
    )?))
}

fn runtime_page_shell_template(
    namespace: TemplateNamespace,
) -> Result<TemplateDefinition, TemplateModelError> {
    let title = ElementNode::new("title", vec![Node::value("page_title")?])?;
    let head = ElementNode::new("head", vec![Node::Element(title)])?;
    let body = ElementNode::new("body", vec![Node::raw_value("page_content")?])?
        .with_attribute(AttributeNode::dynamic_text(
            "data-customer-app",
            "customer_app",
        )?)
        .with_attribute(AttributeNode::dynamic_text("data-route", "route_name")?)
        .with_attribute(AttributeNode::dynamic_text(
            "data-template",
            "template_name",
        )?);
    let html = ElementNode::new("html", vec![Node::Element(head), Node::Element(body)])?
        .with_attribute(AttributeNode::dynamic_text("lang", "locale")?);

    Ok(TemplateDefinition::layout(
        namespace,
        TemplateName::new("runtime.page.shell")?,
        vec![Node::static_text("<!DOCTYPE html>"), Node::Element(html)],
    ))
}

fn runtime_fallback_page_template(
    namespace: TemplateNamespace,
    name: TemplateName,
) -> Result<TemplateDefinition, TemplateModelError> {
    let heading = ElementNode::new("h1", vec![Node::value("route_name")?])?;
    let path = ElementNode::new("p", vec![Node::value("path")?])?.with_attribute(
        AttributeNode::static_value("class", "davenda-runtime-path")?,
    );
    let template = ElementNode::new("p", vec![Node::value("template_name")?])?.with_attribute(
        AttributeNode::static_value("class", "davenda-runtime-template")?,
    );
    let main = ElementNode::new(
        "main",
        vec![
            Node::Element(heading),
            Node::Element(path),
            Node::Element(template),
        ],
    )?
    .with_attribute(AttributeNode::dynamic_text("data-route", "route_name")?)
    .with_attribute(AttributeNode::dynamic_text(
        "data-template",
        "template_name",
    )?);

    let body = ElementNode::new("body", vec![Node::Element(main)])?;
    let html = ElementNode::new("html", vec![Node::Element(body)])?
        .with_attribute(AttributeNode::dynamic_text("lang", "locale")?);

    Ok(TemplateDefinition::layout(
        namespace,
        name,
        vec![Node::static_text("<!DOCTYPE html>"), Node::Element(html)],
    ))
}

fn runtime_fallback_fragment_template(
    namespace: TemplateNamespace,
    name: TemplateName,
) -> Result<TemplateDefinition, TemplateModelError> {
    let heading = ElementNode::new("strong", vec![Node::value("route_name")?])?;
    let path = ElementNode::new("span", vec![Node::value("path")?])?;
    let container = ElementNode::new(
        "div",
        vec![
            Node::Element(heading),
            Node::static_text(" "),
            Node::Element(path),
        ],
    )?
    .with_attribute(AttributeNode::dynamic_text("id", "surface_id")?)
    .with_attribute(AttributeNode::dynamic_text(
        "data-template",
        "template_name",
    )?)
    .with_attribute(AttributeNode::dynamic_text("data-locale", "locale")?);

    Ok(TemplateDefinition::fragment(
        namespace,
        name,
        vec![Node::Element(container)],
    ))
}
