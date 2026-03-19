use super::*;

impl RuntimeHostServiceExecutor {
    pub(super) fn execute_data(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &DataServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self.data_backend(context)?.execute(request, context)?;
        Ok(self.host_service_execution(call, HostServiceResult::Data(execution)))
    }

    fn data_backend(
        &self,
        context: &InvocationContext,
    ) -> Result<&RuntimeDataBackend, WasmModelError> {
        let result = self.data_backend.get_or_init(|| {
            RuntimeDataBackend::new(&self.plan).map_err(|reason| reason.to_string())
        });

        result
            .as_ref()
            .map_err(|reason| runtime_data_backend_error(context, reason.clone()))
    }
}
