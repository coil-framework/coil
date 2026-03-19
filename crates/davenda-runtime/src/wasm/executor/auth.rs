use super::*;

impl RuntimeHostServiceExecutor {
    pub(super) fn execute_auth(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &AuthServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let backend = self.auth_backend()?;
        let execution = backend.execute(request, context, self.plan.tenant_id())?;
        Ok(self.host_service_execution(call, HostServiceResult::Auth(execution)))
    }

    fn auth_backend(&self) -> Result<&RuntimeAuthBackend, WasmModelError> {
        let result = self.auth_backend.get_or_init(|| {
            RuntimeAuthBackend::new(&self.plan).map_err(|reason| {
                runtime_auth_backend_error(self.plan.tenant_id(), reason).to_string()
            })
        });

        result.as_ref().map_err(|reason: &String| {
            runtime_auth_backend_error(self.plan.tenant_id(), reason.clone())
        })
    }
}
