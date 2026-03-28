use super::*;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

mod authorizer;
mod explain;
mod request;
#[cfg(test)]
mod testing;

pub(crate) use authorizer::DeferredPostgresRouteCapabilityAuthorizer;
pub(crate) use coil_auth::LiveAuthExplainRequest;
pub(crate) use explain::auth_explain_router;
pub(crate) use request::authorize_live_request;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use testing::{
    LiveAuthorizationCheck, StaticLiveAuthExplainer, StaticLiveRouteCapabilityAuthorizer,
};

pub(super) type RouteAuthorizationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<bool, RuntimeServerError>> + Send + 'a>>;

pub(super) type AuthExplainFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<coil_auth::CapabilityExplanation, RuntimeServerError>>
            + Send
            + 'a,
    >,
>;

pub(crate) trait LiveRouteCapabilityAuthorizer: Send + Sync {
    fn check_capability<'a>(
        &'a self,
        subject: &'a coil_auth::DefaultSubject,
        capability: coil_auth::Capability,
        object: &'a coil_auth::Entity,
    ) -> RouteAuthorizationFuture<'a>;
}

pub(crate) trait LiveAuthExplainer: Send + Sync {
    fn explain_capability<'a>(
        &'a self,
        request: &'a LiveAuthExplainRequest,
    ) -> AuthExplainFuture<'a>;
}

impl LiveAuthExplainer for coil_auth::LiveAuthExplainHost {
    fn explain_capability<'a>(
        &'a self,
        request: &'a LiveAuthExplainRequest,
    ) -> AuthExplainFuture<'a> {
        Box::pin(async move {
            coil_auth::LiveAuthExplainHost::explain_capability(self, request)
                .await
                .map_err(|error| RuntimeServerError::Explain {
                    reason: error.to_string(),
                })
        })
    }
}

impl fmt::Debug for dyn LiveRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LiveRouteCapabilityAuthorizer")
    }
}

impl fmt::Debug for dyn LiveAuthExplainer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LiveAuthExplainer")
    }
}
