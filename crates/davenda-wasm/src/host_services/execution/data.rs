use crate::host_api::DataServiceRequest;
use crate::invocation::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataServiceExecution {
    pub request: DataServiceRequest,
    pub summary: String,
    pub sequence: u64,
}

pub(super) fn data_summary_for_request(
    request: &DataServiceRequest,
    _context: &InvocationContext,
    sequence: u64,
) -> String {
    match request {
        DataServiceRequest::Read { contract } => contract.summary("read", sequence),
        DataServiceRequest::Write { contract } => contract.summary("write", sequence),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_api::ModuleDataContract;
    use crate::invocation::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef,
        TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("data-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-data").unwrap(),
            InvocationInput::Api(ApiInvocation::new("/data", crate::ids::HttpMethod::Get).unwrap()),
        )
    }

    #[test]
    fn data_summary_uses_the_contract_summary() {
        let contract =
            ModuleDataContract::new("extension-1", "handler-1", "customer.profile").unwrap();
        let request = DataServiceRequest::Read { contract };
        let summary = data_summary_for_request(&request, &context(), 3);
        assert!(summary.contains("read"));
        assert!(summary.contains("3"));
    }
}
