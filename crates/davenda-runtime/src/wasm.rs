use super::*;

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
}

impl WasmHost {
    pub(crate) fn new(
        customer_app: String,
        runtime: WasmRuntimeServices,
        registry: ExtensionRegistry,
        default_locale: String,
        registered_jobs: Vec<RuntimeJobDefinition>,
    ) -> Self {
        Self {
            customer_app,
            runtime,
            registry,
            engine: WasmEngine::new(),
            default_locale,
            registered_jobs,
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
            .map(InvocationPlan::begin_execution))
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
            .map(InvocationPlan::begin_execution))
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
            .map(InvocationPlan::begin_execution))
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
            .map(InvocationPlan::begin_execution))
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
            .map(InvocationPlan::begin_execution))
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
            .map(InvocationPlan::begin_execution))
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
            .map(InvocationPlan::begin_execution)
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
            .map(InvocationPlan::begin_execution)
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
