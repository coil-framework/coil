use crate::host_api::AuthServiceRequest;
use crate::invocation::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthServiceExecution {
    pub request: AuthServiceRequest,
    pub allowed: bool,
    pub checks_seen: u32,
    pub principal_id: Option<String>,
    pub details: AuthServiceDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthServiceDetails {
    Check {
        capability: String,
        object: String,
        decision: bool,
    },
    List {
        capability: String,
        namespace: String,
        object_ids: Vec<String>,
    },
    Lookup {
        capability: String,
        object: String,
        relation: String,
        subject_namespace: String,
        subject_ids: Vec<String>,
    },
    TupleWrite {
        capability: String,
        object: String,
        relation: String,
        subject: String,
        updates: Vec<String>,
        written: usize,
    },
}

pub(super) fn auth_details_for_request(
    request: &AuthServiceRequest,
    context: &InvocationContext,
    sequence: u32,
) -> AuthServiceDetails {
    let principal = context
        .principal
        .id
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let tenant = context
        .customer_app
        .tenant_id
        .clone()
        .unwrap_or_else(|| context.customer_app.app_id.clone());

    match request {
        AuthServiceRequest::Check => AuthServiceDetails::Check {
            capability: "system.config.read".to_string(),
            object: format!("tenant:{tenant}"),
            decision: true,
        },
        AuthServiceRequest::List => AuthServiceDetails::List {
            capability: "cms.page.read".to_string(),
            namespace: "page".to_string(),
            object_ids: vec![format!("synthetic-page-{sequence}")],
        },
        AuthServiceRequest::Lookup => AuthServiceDetails::Lookup {
            capability: "system.module.manage".to_string(),
            object: format!("tenant:{tenant}"),
            relation: "manage".to_string(),
            subject_namespace: "user".to_string(),
            subject_ids: vec![principal],
        },
        AuthServiceRequest::TupleWrite => AuthServiceDetails::TupleWrite {
            capability: "system.config.write".to_string(),
            object: format!("tenant:{tenant}"),
            relation: "manage".to_string(),
            subject: principal,
            updates: vec![format!("tenant:{tenant}#manage")],
            written: 1,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_api::AuthServiceRequest;
    use crate::invocation::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, PrincipalRef,
        TraceContext,
    };

    fn context() -> InvocationContext {
        InvocationContext::new(
            CustomerAppContext::new("auth-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-auth").unwrap(),
            InvocationInput::Api(ApiInvocation::new("/auth", crate::ids::HttpMethod::Get).unwrap()),
        )
    }

    #[test]
    fn auth_details_are_stable_for_checks() {
        let details = auth_details_for_request(&AuthServiceRequest::Check, &context(), 1);
        match details {
            AuthServiceDetails::Check {
                capability,
                object,
                decision,
            } => {
                assert_eq!(capability, "system.config.read");
                assert_eq!(object, "tenant:101");
                assert!(decision);
            }
            other => panic!("unexpected details: {other:?}"),
        }
    }
}
