use std::collections::BTreeMap;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};

use coil_core::{DataRepositoryContribution, DataRepositoryPrincipalBinding};
use coil_data::{DataModelError, DataRuntime, PostgresDataClient, QueryContext, QuerySpec};
use coil_wasm::{
    DataServiceExecution, DataServiceRequest, HostServiceDomain, ModuleDataContract, WasmModelError,
};

use crate::{InvocationContext, RuntimePlan};

#[derive(Debug)]
pub(crate) struct RuntimeDataBackend {
    client: Option<PostgresDataClient>,
    repositories: BTreeMap<String, DataRepositoryContribution>,
    sequence: AtomicU64,
}

impl RuntimeDataBackend {
    pub(crate) fn new(plan: &RuntimePlan) -> Result<Self, String> {
        let repositories = plan
            .module_data_repositories
            .iter()
            .map(|registered| {
                (
                    registered.contribution.id.clone(),
                    registered.contribution.clone(),
                )
            })
            .collect();

        match plan.data.connect_lazy_postgres() {
            Ok(client) => Ok(Self {
                client: Some(client),
                repositories,
                sequence: AtomicU64::new(0),
            }),
            Err(error) => {
                #[cfg(test)]
                {
                    let _ = error;
                    Ok(Self {
                        client: None,
                        repositories,
                        sequence: AtomicU64::new(0),
                    })
                }
                #[cfg(not(test))]
                {
                    Err(error.to_string())
                }
            }
        }
    }

    pub(crate) fn execute(
        &self,
        request: &DataServiceRequest,
        context: &InvocationContext,
    ) -> Result<DataServiceExecution, WasmModelError> {
        let sequence = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        match request {
            DataServiceRequest::Read { contract } => {
                let repository = self.repository(contract).map_err(|reason| {
                    unsupported_data_contract(context, contract, "read", reason)
                })?;
                let summary = if let Some(client) = &self.client {
                    let compiled = compile_query(repository, contract, context, &client.runtime)
                        .map_err(|error| data_execution_error(context, error))?;
                    let execution = block_on_data(client.execute_query(&compiled))
                        .map_err(|error| data_execution_error(context, error))?;
                    format!(
                        "{} rows={} columns={}",
                        contract.summary("read", sequence),
                        execution.rows_returned,
                        execution.projected_columns.join("|")
                    )
                } else {
                    format!("{} synthetic=true", contract.summary("read", sequence))
                };

                Ok(DataServiceExecution {
                    request: request.clone(),
                    summary,
                    sequence,
                })
            }
            DataServiceRequest::Write { contract } => Err(unsupported_data_contract(
                context,
                contract,
                "write",
                "no mutation binding is configured for this repository contract".to_string(),
            )),
        }
    }

    fn repository(
        &self,
        contract: &ModuleDataContract,
    ) -> Result<&DataRepositoryContribution, String> {
        self.repositories
            .get(contract.repository_id())
            .ok_or_else(|| {
                format!(
                    "repository `{}` is not bound to a runtime-managed data service",
                    contract.repository_id()
                )
            })
    }
}

fn compile_query(
    repository: &DataRepositoryContribution,
    contract: &ModuleDataContract,
    context: &InvocationContext,
    runtime: &DataRuntime,
) -> Result<coil_data::CompiledQuery, DataModelError> {
    let query = query_spec(repository, context);
    runtime
        .compile_query(&repository.repository, &query)
        .map_err(|error| DataModelError::Sqlx {
            reason: format!(
                "failed to compile repository `{}` for contract `{}`: {error}",
                repository.id, contract.resource
            ),
        })
}

fn query_spec(repository: &DataRepositoryContribution, context: &InvocationContext) -> QuerySpec {
    let locale = context.customer_app.locale.clone();
    let principal_id = match repository.query_profile.principal_binding {
        DataRepositoryPrincipalBinding::Omit => None,
        DataRepositoryPrincipalBinding::InvocationPrincipal => context.principal.id.clone(),
    };
    let cache_scope = if locale.is_some() {
        repository.query_profile.localized_cache_scope
    } else {
        repository.query_profile.default_cache_scope
    };

    QuerySpec::new(
        repository.query_profile.page,
        QueryContext {
            locale,
            principal_id,
            publication_visibility: repository.query_profile.publication_visibility,
            cache_scope,
        },
    )
}

fn block_on_data<T>(
    future: impl Future<Output = Result<T, DataModelError>> + Send,
) -> Result<T, DataModelError>
where
    T: Send,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| DataModelError::Sqlx {
                    reason: error.to_string(),
                })?;
            runtime.block_on(future)
        }
    }
}

fn data_execution_error(context: &InvocationContext, error: impl ToString) -> WasmModelError {
    WasmModelError::EngineTrap {
        handler_id: context.trace.trace_id.clone(),
        reason: error.to_string(),
    }
}

fn unsupported_data_contract(
    context: &InvocationContext,
    contract: &ModuleDataContract,
    access: &str,
    reason: String,
) -> WasmModelError {
    WasmModelError::HostServiceUnavailable {
        handler_id: context.trace.trace_id.clone(),
        domain: HostServiceDomain::Data,
        reason: format!(
            "repository `{}` cannot service `{access}` for handler `{}`: {reason}",
            contract.repository_id(),
            contract.owner_handler_id,
        ),
    }
}
