use super::*;
use std::fmt;
use std::future::Future;
use std::pin::Pin;

mod authorizer;
mod request;
#[cfg(test)]
mod testing;

pub(crate) use authorizer::DeferredPostgresRouteCapabilityAuthorizer;
pub(crate) use request::authorize_live_request;
#[cfg(test)]
pub(crate) use testing::{LiveAuthorizationCheck, StaticLiveRouteCapabilityAuthorizer};

pub(super) type RouteAuthorizationFuture<'a> =
    Pin<Box<dyn Future<Output = Result<bool, RuntimeServerError>> + Send + 'a>>;

pub(crate) trait LiveRouteCapabilityAuthorizer: Send + Sync {
    fn check_capability<'a>(
        &'a self,
        subject: &'a davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &'a davenda_auth::Entity,
    ) -> RouteAuthorizationFuture<'a>;
}

impl fmt::Debug for dyn LiveRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LiveRouteCapabilityAuthorizer")
    }
}
