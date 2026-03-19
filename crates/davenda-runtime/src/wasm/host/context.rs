use super::*;

impl WasmHost {
    pub(super) fn request_context(
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

    pub(super) fn async_context(
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

    pub(super) fn customer_app_context(
        &self,
        locale: Option<&str>,
    ) -> Result<CustomerAppContext, WasmModelError> {
        let mut customer_app = CustomerAppContext::new(self.customer_app.clone())?;
        customer_app = customer_app.with_tenant_id(self.tenant_id.to_string())?;
        customer_app =
            customer_app.with_locale(locale.unwrap_or(self.default_locale.as_str()).to_string())?;
        Ok(customer_app)
    }
}
