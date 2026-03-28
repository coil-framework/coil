use crate::host_api::RenderServiceRequest;
use crate::invocation::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderServiceExecution {
    pub request: RenderServiceRequest,
    pub fragment: String,
}

pub(super) fn render_fragment_for_request(
    request: &RenderServiceRequest,
    context: &InvocationContext,
) -> String {
    let locale = context.customer_app.locale.as_deref().unwrap_or("unknown");
    match request {
        RenderServiceRequest::Fragment { slot } => format!(
            "<div data-coil-slot=\"{slot}\" data-app=\"{}\" data-locale=\"{}\"></div>",
            context.customer_app.app_id, locale
        ),
    }
}

pub(super) fn render_fragment_key(
    request: &RenderServiceRequest,
    context: &InvocationContext,
) -> String {
    match request {
        RenderServiceRequest::Fragment { slot } => {
            format!("{slot}:{}", context.customer_app.app_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_api::RenderServiceRequest;
    use crate::invocation::ApiInvocation;
    use crate::invocation::{
        CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef, TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("render-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-render").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/render", crate::ids::HttpMethod::Get).unwrap(),
            ),
        )
    }

    #[test]
    fn render_fragment_includes_the_slot_and_locale() {
        let request = RenderServiceRequest::Fragment {
            slot: "hero".to_string(),
        };
        let html = render_fragment_for_request(&request, &context());
        assert!(html.contains("data-coil-slot=\"hero\""));
        assert!(html.contains("data-locale=\"en-GB\""));
    }
}
