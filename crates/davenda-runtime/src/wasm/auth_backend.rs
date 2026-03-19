use super::support::{
    block_on_auth, describe_tuple_update, runtime_auth_backend_error, subject_for_description,
    subject_for_principal, synthetic_auth_execution, tenant_object,
};
use super::*;
use zanzibar::RebacEngine;

pub(super) struct RuntimeAuthBackend<E = zanzibar::postgres::PostgresRebacEngine> {
    auth: Option<davenda_auth::DavendaAuth<E>>,
    package: davenda_auth::AuthModelPackageSelection,
}

impl<E> std::fmt::Debug for RuntimeAuthBackend<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeAuthBackend")
            .field(
                "tenant_id",
                &self.auth.as_ref().map(|auth| auth.tenant_id()),
            )
            .field("auth_package", &self.package.manifest().name)
            .finish()
    }
}

impl RuntimeAuthBackend<zanzibar::postgres::PostgresRebacEngine> {
    pub(super) fn new(plan: &RuntimePlan) -> Result<Self, String> {
        let package = plan.auth_package.clone();

        match plan.data.connect_lazy_postgres() {
            Ok(client) => {
                let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
                Ok(Self {
                    auth: Some(davenda_auth::DavendaAuth::new(engine, plan.tenant_id())),
                    package,
                })
            }
            Err(error) => {
                #[cfg(test)]
                {
                    let _ = error;
                    Ok(Self {
                        auth: None,
                        package,
                    })
                }
                #[cfg(not(test))]
                {
                    Err(error.to_string())
                }
            }
        }
    }
}

#[cfg(test)]
impl<E> RuntimeAuthBackend<E>
where
    E: RebacEngine + Clone,
{
    pub(crate) fn from_auth(
        auth: davenda_auth::DavendaAuth<E>,
        package: davenda_auth::AuthModelPackageSelection,
    ) -> Self {
        Self {
            auth: Some(auth),
            package,
        }
    }
}

impl<E> RuntimeAuthBackend<E>
where
    E: RebacEngine + Clone,
{
    #[cfg(test)]
    pub(super) fn package(&self) -> &dyn davenda_auth::AuthModelPackage {
        self.package.package()
    }

    pub(super) fn execute(
        &self,
        request: &AuthServiceRequest,
        context: &InvocationContext,
        tenant_id: i64,
    ) -> Result<AuthServiceExecution, WasmModelError> {
        let subject = subject_for_principal(context);
        let tenant = tenant_object(context, tenant_id);
        let principal_id = context.principal.id.clone();
        let Some(auth) = self.auth.as_ref() else {
            return Ok(synthetic_auth_execution(request, context, tenant_id));
        };

        let auth = auth.clone();
        match request {
            AuthServiceRequest::Check => self.execute_check(auth, subject, tenant, principal_id),
            AuthServiceRequest::List => self.execute_list(auth, subject, principal_id),
            AuthServiceRequest::Lookup => self.execute_lookup(auth, tenant, principal_id),
            AuthServiceRequest::TupleWrite => {
                self.execute_tuple_write(auth, subject, tenant, principal_id)
            }
        }
        .map_err(|reason| runtime_auth_backend_error(tenant_id, reason))
    }

    fn execute_check(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        subject: DefaultSubject,
        tenant: Entity,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::SystemConfigRead;
        let package = self.package.package();
        block_on_auth(async move {
            let allowed = auth
                .check_capability(package, &subject, capability, &tenant)
                .await
                .map_err(|error| error.to_string())?;

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::Check,
                allowed,
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::Check {
                    capability: capability.to_string(),
                    object: tenant.to_string(),
                    decision: allowed,
                },
            })
        })
    }

    fn execute_list(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        subject: DefaultSubject,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::CmsPageRead;
        let binding = self
            .package
            .package()
            .binding_for(capability)
            .ok_or_else(|| format!("no capability binding for `{capability}`"))?;
        let namespace = *binding
            .resource_namespaces
            .first()
            .ok_or_else(|| format!("no resource namespace binding for `{capability}`"))?;
        let relation = binding.relation;

        block_on_auth(async move {
            let object_ids = auth
                .list_objects(&subject, relation, namespace)
                .await
                .map_err(|error| error.to_string())?;

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::List,
                allowed: !object_ids.is_empty(),
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::List {
                    capability: capability.to_string(),
                    namespace: namespace.to_string(),
                    object_ids,
                },
            })
        })
    }

    fn execute_lookup(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        tenant: Entity,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::SystemModuleManage;
        let binding = self
            .package
            .package()
            .binding_for(capability)
            .ok_or_else(|| format!("no capability binding for `{capability}`"))?;
        let relation = binding.relation;

        block_on_auth(async move {
            let subject_ids = auth
                .list_subject_ids(&tenant, relation, Namespace::User)
                .await
                .map_err(|error| error.to_string())?;

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::Lookup,
                allowed: !subject_ids.is_empty(),
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::Lookup {
                    capability: capability.to_string(),
                    object: tenant.to_string(),
                    relation: relation.to_string(),
                    subject_namespace: Namespace::User.to_string(),
                    subject_ids,
                },
            })
        })
    }

    fn execute_tuple_write(
        &self,
        auth: davenda_auth::DavendaAuth<E>,
        subject: DefaultSubject,
        tenant: Entity,
        principal_id: Option<String>,
    ) -> Result<AuthServiceExecution, String> {
        let capability = Capability::SystemConfigWrite;
        let binding = self
            .package
            .package()
            .binding_for(capability)
            .ok_or_else(|| format!("no capability binding for `{capability}`"))?;
        let relation = binding.relation;
        let package = self.package.package();

        block_on_auth(async move {
            let allowed = auth
                .check_capability(package, &subject, capability, &tenant)
                .await
                .map_err(|error| error.to_string())?;
            let updates = vec![DefaultTupleUpdate::Write(DefaultTuple::new(
                tenant.clone(),
                relation,
                subject.clone(),
            ))];
            let written = if allowed {
                auth.write(updates.clone())
                    .await
                    .map_err(|error| error.to_string())?;
                updates.len()
            } else {
                0
            };

            Ok(AuthServiceExecution {
                request: AuthServiceRequest::TupleWrite,
                allowed,
                checks_seen: 1,
                principal_id,
                details: AuthServiceDetails::TupleWrite {
                    capability: capability.to_string(),
                    object: tenant.to_string(),
                    relation: relation.to_string(),
                    subject: subject_for_description(&subject),
                    updates: updates.iter().map(describe_tuple_update).collect(),
                    written,
                },
            })
        })
    }
}

#[cfg(test)]
mod tests;
