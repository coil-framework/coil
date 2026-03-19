use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use davenda_config::StorageClass;
use davenda_data::{
    CompiledQuery, CompiledTransaction, DomainWrite, MutationAction, MutationSpec, PageRequest,
    PublicationVisibility, QueryCacheScope, QueryContext, QueryField, QuerySpec, RepositorySpec,
    TableName, TransactionIsolation, TransactionPlan,
};
use davenda_template::{
    AttributeNode, ElementNode, FragmentRenderRequest, Node, RenderModel, RenderValue,
    TemplateDefinition, TemplateName, TemplateRuntime, TemplateSelector,
};
use davenda_wasm::{
    AuthServiceExecution, AuthServiceRequest, CacheIntentExecution, CacheIntentServiceRequest,
    DataServiceExecution, DataServiceRequest, HostServiceCall, HostServiceExecution,
    HostServiceExecutor, HostServiceRequest, HostServiceResult, JobExecution, MetadataExecution,
    NetworkExecution, RenderServiceExecution, RenderServiceRequest, SecretExecution,
    StorageClassGrant, StorageServiceExecution, StorageServiceRequest,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LiveWasmExecutionError {
    #[error(transparent)]
    Model(#[from] WasmModelError),
    #[error(
        "failed to read installed extension artifact for `{extension_id}` at `{path}`: {reason}"
    )]
    ArtifactRead {
        extension_id: String,
        path: String,
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredExtensionSlot {
    pub module: String,
    pub kind: ExtensionPointKind,
    pub surface: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtensionSummary {
    pub extension_id: String,
    pub display_name: String,
    pub customer_app_id: String,
    pub handler_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionPrincipal {
    Anonymous,
    User(String),
    ServiceAccount(String),
}

impl ExtensionPrincipal {
    pub fn anonymous() -> Self {
        Self::Anonymous
    }

    pub fn user(id: impl Into<String>) -> Self {
        Self::User(id.into())
    }

    pub fn service_account(id: impl Into<String>) -> Self {
        Self::ServiceAccount(id.into())
    }

    fn to_wasm_principal(&self) -> Result<PrincipalRef, WasmModelError> {
        match self {
            Self::Anonymous => Ok(PrincipalRef::anonymous()),
            Self::User(id) => PrincipalRef::user(id.clone()),
            Self::ServiceAccount(id) => PrincipalRef::service_account(id.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WasmHost {
    pub customer_app: String,
    pub runtime: WasmRuntimeServices,
    registry: ExtensionRegistry,
    engine: WasmEngine,
    default_locale: String,
    registered_jobs: Vec<RuntimeJobDefinition>,
    host_service_executor: Arc<dyn HostServiceExecutor>,
}

impl WasmHost {
    pub(crate) fn new(
        plan: RuntimePlan,
        customer_app: String,
        runtime: WasmRuntimeServices,
        registry: ExtensionRegistry,
        default_locale: String,
        registered_jobs: Vec<RuntimeJobDefinition>,
    ) -> Self {
        let host_service_executor = Arc::new(RuntimeHostServiceExecutor::new(plan.clone()));
        Self {
            customer_app,
            runtime,
            registry,
            engine: WasmEngine::new(),
            default_locale,
            registered_jobs,
            host_service_executor,
        }
    }

    pub fn compile_module(&self, bytes: &[u8]) -> Result<CompiledWasmModule, WasmModelError> {
        self.engine.compile_module(bytes)
    }

    pub fn execute_session(
        &self,
        module: &CompiledWasmModule,
        session: WasmExecutionSession,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        let export = self
            .registry
            .handler_export(&session.plan().extension_id, &session.plan().handler_id)
            .ok_or_else(|| WasmModelError::HandlerNotFound {
                handler_id: session.plan().handler_id.to_string(),
            })?;
        self.engine.execute_session(module, session, export)
    }

    pub fn execute_request_surface(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<ExecutionReceipt>, LiveWasmExecutionError> {
        match execution.route_area {
            RouteArea::Api => self
                .begin_api_invocation(execution)?
                .map(|session| self.execute_installed_session(session))
                .transpose(),
            _ => self
                .begin_page_invocation(execution)?
                .map(|session| self.execute_installed_session(session))
                .transpose(),
        }
    }

    pub fn execute_render_hook_slot(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<ExecutionReceipt>, LiveWasmExecutionError> {
        let sessions = self.begin_render_hook_invocations(slot, execution)?;
        let mut receipts = Vec::with_capacity(sessions.len());
        for session in sessions {
            receipts.push(self.execute_installed_session(session)?);
        }
        Ok(receipts)
    }

    pub fn execute_admin_widget_slot(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<ExecutionReceipt>, LiveWasmExecutionError> {
        let sessions = self.begin_admin_widget_invocations(slot, execution)?;
        let mut receipts = Vec::with_capacity(sessions.len());
        for session in sessions {
            receipts.push(self.execute_installed_session(session)?);
        }
        Ok(receipts)
    }

    fn execute_installed_session(
        &self,
        session: WasmExecutionSession,
    ) -> Result<ExecutionReceipt, LiveWasmExecutionError> {
        let module = self.load_installed_module(&session.plan().extension_id)?;
        self.execute_session(&module, session).map_err(Into::into)
    }

    pub fn prepare_page_invocation(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let method = http_method_to_wasm(execution.method);
        let input = InvocationInput::Page(PageInvocation::new(execution.path.clone(), method)?);
        let context = self.request_context(execution, input)?;
        self.registry
            .prepare_page_invocation(&execution.path, method, context)
    }

    pub fn begin_page_invocation(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_page_invocation(execution)?
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone())))
    }

    pub fn prepare_api_invocation(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let method = http_method_to_wasm(execution.method);
        let input = InvocationInput::Api(ApiInvocation::new(execution.path.clone(), method)?);
        let context = self.request_context(execution, input)?;
        self.registry
            .prepare_api_invocation(&execution.path, method, context)
    }

    pub fn begin_api_invocation(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_api_invocation(execution)?
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone())))
    }

    pub fn prepare_leased_job_invocation(
        &self,
        lease: &JobLease,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let trace_id = format!("job:{}", lease.record.spec.job_id.as_str());
        let principal = ExtensionPrincipal::service_account("runtime.jobs");
        let attempts = lease.record.attempts.saturating_add(1);
        let job_name = lease.record.spec.job_name.as_str();

        if let Some(declared_job) = job_name.strip_prefix("event-handler:") {
            return self.prepare_job_invocation(declared_job, attempts, trace_id, principal);
        }

        match self
            .registered_jobs
            .iter()
            .find(|definition| definition.contract.name == job_name)
            .map(|definition| definition.contract.trigger)
        {
            Some(JobTriggerKind::Scheduled) => {
                self.prepare_scheduled_job_invocation(job_name, trace_id, principal)
            }
            _ => self.prepare_job_invocation(job_name, attempts, trace_id, principal),
        }
    }

    pub fn begin_leased_job_invocation(
        &self,
        lease: &JobLease,
    ) -> Result<Option<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_leased_job_invocation(lease)?
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone())))
    }

    pub fn prepare_job_invocation(
        &self,
        job_name: &str,
        attempt: u32,
        trace_id: impl Into<String>,
        principal: ExtensionPrincipal,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let input = InvocationInput::Job(JobInvocation::new(job_name.to_string(), attempt)?);
        let context = self.async_context(trace_id.into(), principal, input)?;
        self.registry.prepare_job_invocation(job_name, context)
    }

    pub fn begin_job_invocation(
        &self,
        job_name: &str,
        attempt: u32,
        trace_id: impl Into<String>,
        principal: ExtensionPrincipal,
    ) -> Result<Option<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_job_invocation(job_name, attempt, trace_id, principal)?
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone())))
    }

    pub fn prepare_scheduled_job_invocation(
        &self,
        job_name: &str,
        trace_id: impl Into<String>,
        principal: ExtensionPrincipal,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let input =
            InvocationInput::ScheduledJob(ScheduledJobInvocation::new(job_name.to_string())?);
        let context = self.async_context(trace_id.into(), principal, input)?;
        self.registry
            .prepare_scheduled_job_invocation(job_name, context)
    }

    pub fn begin_scheduled_job_invocation(
        &self,
        job_name: &str,
        trace_id: impl Into<String>,
        principal: ExtensionPrincipal,
    ) -> Result<Option<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_scheduled_job_invocation(job_name, trace_id, principal)?
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone())))
    }

    pub fn prepare_webhook_invocation(
        &self,
        source: &str,
        event: &str,
        verified: bool,
        replay_protected: bool,
        trace_id: impl Into<String>,
        principal: ExtensionPrincipal,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let input = InvocationInput::Webhook(WebhookInvocation::new(
            source.to_string(),
            event.to_string(),
            verified,
            replay_protected,
        )?);
        let context = self.async_context(trace_id.into(), principal, input)?;
        self.registry
            .prepare_webhook_invocation(source, event, context)
    }

    pub fn begin_webhook_invocation(
        &self,
        source: &str,
        event: &str,
        verified: bool,
        replay_protected: bool,
        trace_id: impl Into<String>,
        principal: ExtensionPrincipal,
    ) -> Result<Option<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_webhook_invocation(
                source,
                event,
                verified,
                replay_protected,
                trace_id,
                principal,
            )?
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone())))
    }

    pub fn prepare_admin_widget_invocations(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<InvocationPlan>, WasmModelError> {
        let input = InvocationInput::AdminWidget(AdminWidgetInvocation::new(slot.to_string())?);
        let context = self.request_context(execution, input)?;
        self.registry
            .prepare_admin_widget_invocations(slot, context)
    }

    pub fn begin_admin_widget_invocations(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_admin_widget_invocations(slot, execution)?
            .into_iter()
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone()))
            .collect())
    }

    pub fn prepare_render_hook_invocations(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<InvocationPlan>, WasmModelError> {
        let input = InvocationInput::RenderHook(RenderHookInvocation::new(slot.to_string())?);
        let context = self.request_context(execution, input)?;
        self.registry.prepare_render_hook_invocations(slot, context)
    }

    pub fn begin_render_hook_invocations(
        &self,
        slot: &str,
        execution: &RequestExecution,
    ) -> Result<Vec<WasmExecutionSession>, WasmModelError> {
        Ok(self
            .prepare_render_hook_invocations(slot, execution)?
            .into_iter()
            .map(|plan| plan.begin_execution_with_executor(self.host_service_executor.clone()))
            .collect())
    }

    fn request_context(
        &self,
        execution: &RequestExecution,
        input: InvocationInput,
    ) -> Result<InvocationContext, WasmModelError> {
        let customer_app = self.customer_app_context(Some(&execution.locale))?;
        let principal = match execution.principal.principal_id.as_deref() {
            Some(principal_id) => PrincipalRef::user(principal_id.to_string())?,
            None => PrincipalRef::anonymous(),
        };
        let trace = TraceContext::new(execution.trace.request_id.clone())?
            .with_request_id(execution.trace.request_id.clone())?;

        Ok(InvocationContext::new(
            customer_app,
            principal,
            trace,
            input,
        ))
    }

    fn async_context(
        &self,
        trace_id: String,
        principal: ExtensionPrincipal,
        input: InvocationInput,
    ) -> Result<InvocationContext, WasmModelError> {
        let customer_app = self.customer_app_context(None)?;
        let principal = principal.to_wasm_principal()?;
        let trace = TraceContext::new(trace_id)?;

        Ok(InvocationContext::new(
            customer_app,
            principal,
            trace,
            input,
        ))
    }

    fn customer_app_context(
        &self,
        locale: Option<&str>,
    ) -> Result<CustomerAppContext, WasmModelError> {
        let mut customer_app = CustomerAppContext::new(self.customer_app.clone())?;
        customer_app =
            customer_app.with_locale(locale.unwrap_or(self.default_locale.as_str()).to_string())?;
        Ok(customer_app)
    }

    fn load_installed_module(
        &self,
        extension_id: &davenda_wasm::ExtensionId,
    ) -> Result<CompiledWasmModule, LiveWasmExecutionError> {
        let bytes = if let Some(installed) = self.registry.extension(extension_id) {
            if let Some(artifact) = installed.artifact() {
                artifact.load_bytes(&self.runtime.extension_directory, extension_id)?
            } else {
                let path = self.installed_module_path(extension_id);
                fs::read(&path).map_err(|error| LiveWasmExecutionError::ArtifactRead {
                    extension_id: extension_id.to_string(),
                    path: path.display().to_string(),
                    reason: error.to_string(),
                })?
            }
        } else {
            let path = self.installed_module_path(extension_id);
            fs::read(&path).map_err(|error| LiveWasmExecutionError::ArtifactRead {
                extension_id: extension_id.to_string(),
                path: path.display().to_string(),
                reason: error.to_string(),
            })?
        };

        self.compile_module(&bytes).map_err(Into::into)
    }

    fn installed_module_path(&self, extension_id: &davenda_wasm::ExtensionId) -> PathBuf {
        PathBuf::from(&self.runtime.extension_directory).join(format!("{extension_id}.wasm"))
    }
}

#[derive(Debug, Clone)]
struct RuntimeHostServiceExecutor {
    plan: RuntimePlan,
}

impl RuntimeHostServiceExecutor {
    fn new(plan: RuntimePlan) -> Self {
        Self { plan }
    }

    fn execute_auth(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &AuthServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let allowed = context.principal.validate().is_ok();
        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Auth(AuthServiceExecution {
                request: request.clone(),
                allowed,
                checks_seen: 1,
                principal_id: context.principal.id.clone(),
            }),
        })
    }

    fn execute_data(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &DataServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let result = match request {
            DataServiceRequest::Read { resource } => {
                let compiled = self.compile_data_read(resource, context)?;
                HostServiceResult::Data(DataServiceExecution {
                    request: request.clone(),
                    statement: compiled.sql,
                    sequence: compiled.bind_values.len() as u64,
                })
            }
            DataServiceRequest::Write { resource } => {
                let compiled = self.compile_data_write(resource, context)?;
                let statement = compiled
                    .statements
                    .first()
                    .map(|statement| statement.sql.clone())
                    .unwrap_or(compiled.commit_sql.clone());
                HostServiceResult::Data(DataServiceExecution {
                    request: request.clone(),
                    statement,
                    sequence: compiled.statements.len() as u64,
                })
            }
        };

        Ok(HostServiceExecution {
            call: call.clone(),
            result,
        })
    }

    fn execute_storage(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &StorageServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let (storage_class, bytes) = match request {
            StorageServiceRequest::Read { class } => (*class, 0),
            StorageServiceRequest::Write { class, bytes } => (*class, *bytes),
        };
        let trace_id = trace_id(context);
        let logical_path = format!(
            "wasm/{}/{}/{}",
            context.customer_app.app_id, trace_id, storage_class
        );
        let plan = self
            .plan
            .storage_host()
            .plan_write(
                StoragePlanRequest::new(logical_path)
                    .with_storage_class(storage_class_from_grant(storage_class)),
            )
            .map_err(|error| runtime_executor_error(context, error))?;
        let description = format!(
            "{} via {}",
            plan.logical_path,
            plan.primary_write_target()
                .map(|target| target.locator.as_str())
                .unwrap_or("local")
        );

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Storage(StorageServiceExecution {
                request: request.clone(),
                description,
                total_bytes: bytes,
            }),
        })
    }

    fn execute_render(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &RenderServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let fragment = self.render_fragment(request, context)?;
        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::Render(RenderServiceExecution {
                request: request.clone(),
                fragment,
            }),
        })
    }

    fn execute_cache_intent(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &CacheIntentServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let trace_id = trace_id(context);
        let cache_namespace = CacheNamespace::new(format!("wasm:{}", context.customer_app.app_id))
            .map_err(|error| runtime_executor_error(context, error))?;
        let mut scope = if context.principal.id.is_some() {
            CacheScope::private()
        } else {
            CacheScope::public()
        };
        if let Some(locale) = context.customer_app.locale.as_deref() {
            scope = scope
                .with_locale(locale.to_string())
                .map_err(|error| runtime_executor_error(context, error))?;
        }
        scope = scope
            .with_site(context.customer_app.app_id.clone())
            .map_err(|error| runtime_executor_error(context, error))?;
        let freshness =
            FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(30)))
                .expect("constant freshness policy is valid");
        let validators = ResponseValidators {
            etag: Some(
                EntityTag::new(format!(
                    "wasm:{}:{}:cache-intent",
                    context.customer_app.app_id, trace_id
                ))
                .map_err(|error| runtime_executor_error(context, error))?,
            ),
            last_modified_unix_seconds: None,
        };
        let surrogate_tags = InvalidationSet::from_tags([
            InvalidationTag::new(format!("app:{}", context.customer_app.app_id))
                .map_err(|error| runtime_executor_error(context, error))?,
            InvalidationTag::new(format!("trace:{}", trace_id))
                .map_err(|error| runtime_executor_error(context, error))?,
        ]);
        let http_policy =
            HttpCachePolicy::new(scope.clone(), Some(freshness), validators, surrogate_tags)
                .map_err(|error| runtime_executor_error(context, error))?;
        let cache_request =
            CachePlanRequest::new(cache_namespace, format!("wasm:{}", trace_id), http_policy)
                .map_err(|error| runtime_executor_error(context, error))?
                .with_application_policy(
                    ApplicationCachePolicy::new(scope, freshness, InvalidationSet::new())
                        .map_err(|error| runtime_executor_error(context, error))?,
                );
        let plan = self
            .plan
            .cache_planner
            .plan(cache_request)
            .map_err(|error| runtime_executor_error(context, error))?;
        let cache_key = plan
            .application()
            .map(|application| application.key().to_string())
            .unwrap_or_else(|| format!("wasm:{}", trace_id));

        Ok(HostServiceExecution {
            call: call.clone(),
            result: HostServiceResult::CacheIntent(CacheIntentExecution {
                request: request.clone(),
                cache_key,
                applied: plan.application().is_some(),
            }),
        })
    }

    fn compile_data_read(
        &self,
        resource: &str,
        context: &InvocationContext,
    ) -> Result<CompiledQuery, WasmModelError> {
        let repository = runtime_repository_spec(resource, context)?;
        let query = QuerySpec::new(
            PageRequest::new(0, 1).map_err(|error| runtime_executor_error(context, error))?,
            QueryContext {
                locale: context.customer_app.locale.clone(),
                principal_id: context.principal.id.clone(),
                publication_visibility: PublicationVisibility::IncludeDrafts,
                cache_scope: if context.principal.id.is_some() {
                    QueryCacheScope::UserScoped
                } else {
                    QueryCacheScope::Public
                },
            },
        );
        self.plan
            .data
            .compile_query(&repository, &query)
            .map_err(|error| runtime_executor_error(context, error))
    }

    fn compile_data_write(
        &self,
        resource: &str,
        context: &InvocationContext,
    ) -> Result<CompiledTransaction, WasmModelError> {
        let transaction = TransactionPlan::new(
            format!("wasm.{resource}.write"),
            TransactionIsolation::ReadCommitted,
        )
        .map_err(|error| runtime_executor_error(context, error))?
        .with_write(
            DomainWrite::new(resource, "write")
                .map_err(|error| runtime_executor_error(context, error))?,
        );
        let mutation = MutationSpec::new(
            format!("wasm_{}", resource.replace(['.', ':', '/'], "_")),
            MutationAction::Insert,
        )
        .map_err(|error| runtime_executor_error(context, error))?
        .with_assignment("resource", resource.to_string())
        .map_err(|error| runtime_executor_error(context, error))?
        .with_assignment("customer_app", context.customer_app.app_id.clone())
        .map_err(|error| runtime_executor_error(context, error))?
        .with_assignment(
            "principal",
            context
                .principal
                .id
                .clone()
                .unwrap_or_else(|| "anonymous".to_string()),
        )
        .map_err(|error| runtime_executor_error(context, error))?;
        self.plan
            .data
            .compile_transaction(&transaction, &[mutation])
            .map_err(|error| runtime_executor_error(context, error))
    }

    fn render_fragment(
        &self,
        request: &RenderServiceRequest,
        context: &InvocationContext,
    ) -> Result<String, WasmModelError> {
        let slot = match request {
            RenderServiceRequest::Fragment { slot } => slot,
        };
        let fragment_name = TemplateName::new(format!("wasm-host-{slot}"))
            .map_err(|error| runtime_executor_error(context, error))?;
        let definition = TemplateDefinition::fragment(
            self.plan.template.customer_app_namespace.clone(),
            fragment_name.clone(),
            vec![Node::Element(
                ElementNode::new(
                    "div",
                    vec![Node::static_text(format!(
                        "host-render:{}:{}",
                        context.customer_app.app_id, slot
                    ))],
                )
                .map_err(|error| runtime_executor_error(context, error))?
                .with_attribute(
                    AttributeNode::static_value("data-slot", slot)
                        .map_err(|error| runtime_executor_error(context, error))?,
                )
                .with_attribute(
                    AttributeNode::static_value("data-app", context.customer_app.app_id.clone())
                        .map_err(|error| runtime_executor_error(context, error))?,
                )
                .with_attribute(
                    AttributeNode::static_value(
                        "data-locale",
                        context
                            .customer_app
                            .locale
                            .clone()
                            .unwrap_or_else(|| self.plan.config.i18n.default_locale.clone()),
                    )
                    .map_err(|error| runtime_executor_error(context, error))?,
                ),
            )],
        );
        let mut registry = self.plan.template.registry.clone();
        registry
            .register(definition)
            .map_err(|error| runtime_executor_error(context, error))?;
        let runtime = TemplateRuntime::new(registry);
        let selector = TemplateSelector::new(fragment_name);
        let model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(context.customer_app.app_id.clone()),
            )
            .map_err(|error| runtime_executor_error(context, error))?
            .with_value("slot", RenderValue::text(slot.clone()))
            .map_err(|error| runtime_executor_error(context, error))?
            .with_value(
                "locale",
                RenderValue::text(
                    context
                        .customer_app
                        .locale
                        .clone()
                        .unwrap_or_else(|| self.plan.config.i18n.default_locale.clone()),
                ),
            )
            .map_err(|error| runtime_executor_error(context, error))?;

        runtime
            .render_fragment(
                &[self.plan.template.customer_app_namespace.clone()],
                FragmentRenderRequest::new(selector, model),
            )
            .map(|output| output.html)
            .map_err(|error| runtime_executor_error(context, error))
    }
}

impl HostServiceExecutor for RuntimeHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError> {
        match &call.request {
            HostServiceRequest::Auth(request) => self.execute_auth(call, context, request),
            HostServiceRequest::Data(request) => self.execute_data(call, context, request),
            HostServiceRequest::Storage(request) => self.execute_storage(call, context, request),
            HostServiceRequest::Render(request) => self.execute_render(call, context, request),
            HostServiceRequest::CacheIntent(request) => {
                self.execute_cache_intent(call, context, request)
            }
            HostServiceRequest::OutboundHttp {
                integration,
                response_bytes,
            } => Ok(HostServiceExecution {
                call: call.clone(),
                result: HostServiceResult::Network(NetworkExecution {
                    integration: integration.clone(),
                    response_bytes: *response_bytes,
                }),
            }),
            HostServiceRequest::SecretRead { secret } => Ok(HostServiceExecution {
                call: call.clone(),
                result: HostServiceResult::Secret(SecretExecution {
                    secret: secret.clone(),
                }),
            }),
            HostServiceRequest::EnqueueJob { queue } => Ok(HostServiceExecution {
                call: call.clone(),
                result: HostServiceResult::Job(JobExecution {
                    queue: queue.clone(),
                }),
            }),
            HostServiceRequest::MetadataWrite { kind } => Ok(HostServiceExecution {
                call: call.clone(),
                result: HostServiceResult::Metadata(MetadataExecution {
                    kind: *kind,
                    recorded: true,
                }),
            }),
        }
    }
}

fn runtime_repository_spec(
    resource: &str,
    context: &InvocationContext,
) -> Result<RepositorySpec, WasmModelError> {
    let repository = RepositorySpec::new(
        format!("wasm.{resource}"),
        TableName::new(format!("wasm_{}", resource.replace(['.', ':', '/'], "_")))
            .map_err(|error| runtime_executor_error(context, error))?,
        vec![
            QueryField::new("id").map_err(|error| runtime_executor_error(context, error))?,
            QueryField::new("resource").map_err(|error| runtime_executor_error(context, error))?,
            QueryField::new("locale").map_err(|error| runtime_executor_error(context, error))?,
        ],
    )
    .map_err(|error| runtime_executor_error(context, error))?
    .with_locale_field("locale")
    .map_err(|error| runtime_executor_error(context, error))?
    .with_filterable_field("resource")
    .map_err(|error| runtime_executor_error(context, error))?
    .with_sortable_field("resource")
    .map_err(|error| runtime_executor_error(context, error))?;

    Ok(repository)
}

fn runtime_executor_error(context: &InvocationContext, error: impl ToString) -> WasmModelError {
    WasmModelError::EngineTrap {
        handler_id: trace_id(context).to_string(),
        reason: error.to_string(),
    }
}

fn trace_id(context: &InvocationContext) -> &str {
    context.trace.request_id.as_deref().unwrap_or("unknown")
}

fn storage_class_from_grant(class: StorageClassGrant) -> StorageClass {
    match class {
        StorageClassGrant::PublicAsset => StorageClass::PublicAsset,
        StorageClassGrant::PublicUpload => StorageClass::PublicUpload,
        StorageClassGrant::PrivateShared => StorageClass::PrivateShared,
        StorageClassGrant::LocalOnlySensitive => StorageClass::LocalOnlySensitive,
    }
}

fn http_method_to_wasm(method: HttpMethod) -> WasmHttpMethod {
    match method {
        HttpMethod::Get => WasmHttpMethod::Get,
        HttpMethod::Head => WasmHttpMethod::Head,
        HttpMethod::Post => WasmHttpMethod::Post,
        HttpMethod::Put => WasmHttpMethod::Put,
        HttpMethod::Patch => WasmHttpMethod::Patch,
        HttpMethod::Delete => WasmHttpMethod::Delete,
    }
}
