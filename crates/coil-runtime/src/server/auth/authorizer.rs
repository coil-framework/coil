use super::*;
use std::fmt;

pub(crate) struct DeferredPostgresRouteCapabilityAuthorizer {
    authorizer: coil_auth::LiveAuthorizationHost,
}

impl DeferredPostgresRouteCapabilityAuthorizer {
    pub(crate) fn new(
        data: DataRuntimeServices,
        tenant_id: i64,
        database_url: Option<String>,
        auth_package: coil_auth::AuthModelPackageSelection,
    ) -> Self {
        let runtime = database_url
            .as_ref()
            .map(|url| data.with_resolved_connection_url(url.clone()))
            .unwrap_or(data);
        Self {
            authorizer: coil_auth::LiveAuthorizationHost::new(runtime, tenant_id, auth_package),
        }
    }
}

impl fmt::Debug for DeferredPostgresRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeferredPostgresRouteCapabilityAuthorizer")
            .field("authorizer", &self.authorizer)
            .finish_non_exhaustive()
    }
}

impl LiveRouteCapabilityAuthorizer for DeferredPostgresRouteCapabilityAuthorizer {
    fn check_capability<'a>(
        &'a self,
        subject: &'a coil_auth::DefaultSubject,
        capability: coil_auth::Capability,
        object: &'a coil_auth::Entity,
    ) -> RouteAuthorizationFuture<'a> {
        Box::pin(async move {
            self.authorizer
                .check_capability(subject, capability, object)
                .await
                .map_err(|error| RuntimeServerError::Authorization {
                    reason: error.to_string(),
                })
        })
    }
}
