use crate::host_api::CacheIntentServiceRequest;
use crate::invocation::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheIntentExecution {
    pub request: CacheIntentServiceRequest,
    pub cache_key: String,
    pub applied: bool,
}

pub(super) fn cache_key_for_request(
    request: &CacheIntentServiceRequest,
    context: &InvocationContext,
    cache_intent_count: usize,
) -> String {
    let _ = request;
    format!(
        "cache-intent:{}:{}:{}",
        context.customer_app.app_id, context.trace.trace_id, cache_intent_count
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_api::CacheIntentServiceRequest;
    use crate::invocation::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef,
        TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("cache-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-cache").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/cache", crate::ids::HttpMethod::Get).unwrap(),
            ),
        )
    }

    #[test]
    fn cache_keys_are_stable_for_the_same_context() {
        let context = context();
        let key = cache_key_for_request(&CacheIntentServiceRequest::HintWrite, &context, 2);
        assert_eq!(key, "cache-intent:cache-app:trace-cache:2");
    }
}
