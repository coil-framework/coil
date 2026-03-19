use std::sync::Arc;

use crate::error::WasmModelError;
use crate::host_api::HostServiceCall;
use crate::invocation::InvocationContext;

use super::types::HostServiceExecution;

pub trait HostServiceExecutor: std::fmt::Debug + Send + Sync {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError>;
}

#[derive(Debug, Clone, Default)]
pub struct DeniedHostServiceExecutor;

impl HostServiceExecutor for DeniedHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        _context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError> {
        Err(WasmModelError::HostServiceUnavailable {
            handler_id: "unknown".to_string(),
            domain: call.domain(),
            reason: "no host service executor configured for this session".to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct HostServiceJournal {
    executor: Arc<dyn HostServiceExecutor>,
    executions: Vec<HostServiceExecution>,
}

impl HostServiceJournal {
    pub fn new() -> Self {
        Self::with_executor(Arc::new(DeniedHostServiceExecutor::default()))
    }

    pub fn with_executor(executor: Arc<dyn HostServiceExecutor>) -> Self {
        Self {
            executor,
            executions: Vec::new(),
        }
    }

    pub fn executions(&self) -> &[HostServiceExecution] {
        &self.executions
    }

    pub fn execute(
        &mut self,
        call: HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let execution = self.executor.execute(&call, context)?;
        self.executions.push(execution.clone());
        Ok(execution)
    }
}

impl Default for HostServiceJournal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grants::HostCapabilityGrant;
    use crate::host_api::{AuthServiceRequest, HostServiceRequest};
    use crate::invocation::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef,
        TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("journal-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-journal").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/journal", crate::ids::HttpMethod::Get).unwrap(),
            ),
        )
    }

    #[test]
    fn denied_executor_rejects_execution() {
        let executor = DeniedHostServiceExecutor;
        let call = crate::host_api::HostServiceCall::new(
            HostCapabilityGrant::AuthCheck,
            HostServiceRequest::Auth(AuthServiceRequest::Check),
        );
        let error = executor.execute(&call, &context()).unwrap_err();
        assert!(matches!(
            error,
            WasmModelError::HostServiceUnavailable { .. }
        ));
    }
}
