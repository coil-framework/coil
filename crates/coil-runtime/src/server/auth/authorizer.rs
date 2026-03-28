use super::*;
use std::fmt;
use std::sync::OnceLock;

pub(crate) struct DeferredPostgresRouteCapabilityAuthorizer {
    data: DataRuntimeServices,
    tenant_id: i64,
    database_url: Option<String>,
    auth_package: coil_auth::AuthModelPackageSelection,
    authorizer: OnceLock<Result<PostgresRouteCapabilityAuthorizer, String>>,
}

impl DeferredPostgresRouteCapabilityAuthorizer {
    pub(crate) fn new(
        data: DataRuntimeServices,
        tenant_id: i64,
        database_url: Option<String>,
        auth_package: coil_auth::AuthModelPackageSelection,
    ) -> Self {
        Self {
            data,
            tenant_id,
            database_url,
            auth_package,
            authorizer: OnceLock::new(),
        }
    }

    fn authorizer(&self) -> Result<&PostgresRouteCapabilityAuthorizer, RuntimeServerError> {
        match self.authorizer.get_or_init(|| self.build_authorizer()) {
            Ok(authorizer) => Ok(authorizer),
            Err(reason) => Err(RuntimeServerError::Authorization {
                reason: reason.clone(),
            }),
        }
    }

    fn build_authorizer(&self) -> Result<PostgresRouteCapabilityAuthorizer, String> {
        let runtime = self
            .database_url
            .as_ref()
            .map(|url| self.data.with_resolved_connection_url(url.clone()))
            .unwrap_or_else(|| self.data.clone());
        let client = runtime
            .connect_lazy_postgres()
            .map_err(|error| error.to_string())?;
        let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());

        Ok(PostgresRouteCapabilityAuthorizer {
            auth: coil_auth::CoilAuth::new(engine, self.tenant_id),
            package: self.auth_package.clone(),
        })
    }
}

impl fmt::Debug for DeferredPostgresRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeferredPostgresRouteCapabilityAuthorizer")
            .field("auth_package", &self.auth_package.manifest().name)
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
            self.authorizer()?
                .check_capability(subject, capability, object)
                .await
        })
    }
}

pub(crate) struct PostgresRouteCapabilityAuthorizer {
    auth: coil_auth::CoilAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: coil_auth::AuthModelPackageSelection,
}

impl PostgresRouteCapabilityAuthorizer {
    async fn check_capability(
        &self,
        subject: &coil_auth::DefaultSubject,
        capability: coil_auth::Capability,
        object: &coil_auth::Entity,
    ) -> Result<bool, RuntimeServerError> {
        self.auth
            .check_capability(self.package.package(), subject, capability, object)
            .await
            .map_err(|error| RuntimeServerError::Authorization {
                reason: error.to_string(),
            })
    }
}

impl fmt::Debug for PostgresRouteCapabilityAuthorizer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostgresRouteCapabilityAuthorizer")
            .field("tenant_id", &self.auth.tenant_id())
            .field("auth_package", &self.package.manifest().name)
            .finish()
    }
}
