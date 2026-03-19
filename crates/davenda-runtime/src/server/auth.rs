use super::*;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};

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

pub(super) struct DeferredPostgresRouteCapabilityAuthorizer {
    data: DataRuntimeServices,
    tenant_id: i64,
    database_url: Option<String>,
    auth_package: davenda_auth::AuthModelPackageSelection,
    authorizer: OnceLock<Result<PostgresRouteCapabilityAuthorizer, String>>,
}

impl DeferredPostgresRouteCapabilityAuthorizer {
    pub(super) fn new(
        data: DataRuntimeServices,
        tenant_id: i64,
        database_url: Option<String>,
        auth_package: davenda_auth::AuthModelPackageSelection,
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
            auth: davenda_auth::DavendaAuth::new(engine, self.tenant_id),
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
        subject: &'a davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &'a davenda_auth::Entity,
    ) -> RouteAuthorizationFuture<'a> {
        Box::pin(async move {
            self.authorizer()?
                .check_capability(subject, capability, object)
                .await
        })
    }
}

struct PostgresRouteCapabilityAuthorizer {
    auth: davenda_auth::DavendaAuth<zanzibar::postgres::PostgresRebacEngine>,
    package: davenda_auth::AuthModelPackageSelection,
}

impl PostgresRouteCapabilityAuthorizer {
    async fn check_capability(
        &self,
        subject: &davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &davenda_auth::Entity,
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

pub(super) async fn authorize_live_request(
    state: &RuntimeServerState,
    request: &mut RequestInput,
) -> Result<(), RuntimeServerError> {
    let matched = state
        .plan
        .http
        .resolve_match(
            &state.plan.config,
            request.method,
            &request.host,
            &request.path,
        )
        .ok_or_else(|| {
            RuntimeServerError::Execution(RequestExecutionError::RouteNotFound {
                method: request.method,
                host: request.host.clone(),
                path: request.path.clone(),
            })
        })?;

    let RouteAuthGate::Capability(capability) = matched.resolved.auth else {
        return Ok(());
    };
    if request.session_id.is_none() {
        return Ok(());
    }

    let Some(principal_id) = request.principal_id.as_deref() else {
        return Ok(());
    };
    let package = state.plan.auth_package.package();
    let module_manifest = matched.route.module.as_deref().and_then(|module_name| {
        state
            .plan
            .modules
            .iter()
            .find(|manifest| manifest.name == module_name)
    });
    let Some(object) = matched
        .resolved
        .capability_auth_resource(&matched.route, module_manifest, package)
        .map_err(|error| RuntimeServerError::Authorization {
            reason: error.to_string(),
        })?
    else {
        return Ok(());
    };
    let subject =
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(principal_id.to_string()));
    let allowed = state
        .route_authorizer
        .check_capability(&subject, capability, &object)
        .await?;

    if allowed {
        request.granted_capabilities.insert(capability);
        Ok(())
    } else {
        Err(RuntimeServerError::Execution(
            RequestExecutionError::CapabilityRequired {
                route: matched.resolved.route_name.clone(),
                capability,
            },
        ))
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LiveAuthorizationCheck {
    pub subject: davenda_auth::DefaultSubject,
    pub capability: davenda_auth::Capability,
    pub object: davenda_auth::Entity,
}

#[cfg(test)]
#[derive(Debug, Clone, Default)]
pub(crate) struct StaticLiveRouteCapabilityAuthorizer {
    allowed: Vec<LiveAuthorizationCheck>,
    checks: Arc<Mutex<Vec<LiveAuthorizationCheck>>>,
}

#[cfg(test)]
impl StaticLiveRouteCapabilityAuthorizer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn allowing(
        mut self,
        subject: davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: davenda_auth::Entity,
    ) -> Self {
        self.allowed.push(LiveAuthorizationCheck {
            subject,
            capability,
            object,
        });
        self
    }

    pub(crate) fn checks(&self) -> Vec<LiveAuthorizationCheck> {
        self.checks
            .lock()
            .expect("static live authorizer mutex poisoned")
            .clone()
    }
}

#[cfg(test)]
impl LiveRouteCapabilityAuthorizer for StaticLiveRouteCapabilityAuthorizer {
    fn check_capability<'a>(
        &'a self,
        subject: &'a davenda_auth::DefaultSubject,
        capability: davenda_auth::Capability,
        object: &'a davenda_auth::Entity,
    ) -> RouteAuthorizationFuture<'a> {
        Box::pin(async move {
            let check = LiveAuthorizationCheck {
                subject: subject.clone(),
                capability,
                object: object.clone(),
            };
            self.checks
                .lock()
                .expect("static live authorizer mutex poisoned")
                .push(check.clone());
            Ok(self.allowed.iter().any(|allowed| allowed == &check))
        })
    }
}
