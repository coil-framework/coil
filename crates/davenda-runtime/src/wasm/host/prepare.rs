use super::*;

impl WasmHost {
    pub fn prepare_page_invocation(
        &self,
        execution: &RequestExecution,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        let method = http_method_to_wasm(execution.method);
        let input = InvocationInput::Page(PageInvocation::new(
            invocation_surface_path(execution),
            method,
        )?);
        let context = self.request_context(execution, input)?;
        self.registry.prepare_page_invocation(
            invocation_surface_path(execution).as_str(),
            method,
            context,
        )
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
        let input = InvocationInput::Api(ApiInvocation::new(
            invocation_surface_path(execution),
            method,
        )?);
        let context = self.request_context(execution, input)?;
        self.registry.prepare_api_invocation(
            invocation_surface_path(execution).as_str(),
            method,
            context,
        )
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
        let prepared = self
            .registry
            .prepare_webhook_invocation(source, event, context.clone());
        if let Err(error) = &prepared {
            let status = match error {
                WasmModelError::UnverifiedWebhook { .. } => {
                    Some(services::WebhookObservationStatus::VerificationFailed)
                }
                WasmModelError::ReplayUnsafeWebhook { .. } => {
                    Some(services::WebhookObservationStatus::ReplayRejected)
                }
                _ => None,
            };
            if let Some(status) = status {
                let _ = self.host_services.record_webhook_observation(
                    source,
                    event,
                    status,
                    &context,
                    Some(error.to_string()),
                );
            }
        }
        prepared
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
}
