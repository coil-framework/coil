use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};

use davenda_data::{
    DataModelError, DataRuntime, PageRequest, PostgresDataClient, PublicationVisibility,
    QueryCacheScope, QueryContext, QueryField, QuerySort, QuerySpec, RepositorySpec, TableName,
};
use davenda_wasm::{
    DataServiceExecution, DataServiceRequest, HostServiceDomain, ModuleDataContract,
    WasmModelError,
};

use crate::{InvocationContext, RuntimePlan};

#[derive(Debug)]
pub(crate) struct RuntimeDataBackend {
    client: Option<PostgresDataClient>,
    sequence: AtomicU64,
}

impl RuntimeDataBackend {
    pub(crate) fn new(plan: &RuntimePlan) -> Result<Self, String> {
        match plan.data.connect_lazy_postgres() {
            Ok(client) => Ok(Self {
                client: Some(client),
                sequence: AtomicU64::new(0),
            }),
            Err(error) => {
                #[cfg(test)]
                {
                    let _ = error;
                    Ok(Self {
                        client: None,
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
                let binding = RuntimeDataReadBinding::from_contract(contract)
                    .map_err(|reason| unsupported_data_contract(context, contract, "read", reason))?;
                let summary = if let Some(client) = &self.client {
                    let compiled = binding
                        .compile_query(contract, context, &client.runtime)
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeDataReadBinding {
    EventsWaitlist,
    CmsPagesLive,
    CommerceCatalogProducts,
}

impl RuntimeDataReadBinding {
    fn from_contract(contract: &ModuleDataContract) -> Result<Self, String> {
        match contract.repository_id() {
            "events.waitlist" => Ok(Self::EventsWaitlist),
            "cms.pages.live" => Ok(Self::CmsPagesLive),
            "commerce.catalog.products" => Ok(Self::CommerceCatalogProducts),
            other => Err(format!(
                "repository `{other}` is not bound to a runtime-managed data service"
            )),
        }
    }

    fn compile_query(
        self,
        contract: &ModuleDataContract,
        context: &InvocationContext,
        runtime: &DataRuntime,
    ) -> Result<davenda_data::CompiledQuery, DataModelError> {
        let repository = self.repository_spec()?;
        let query = self.query_spec(context)?;
        runtime.compile_query(&repository, &query).map_err(|error| {
            DataModelError::Sqlx {
                reason: format!(
                    "failed to compile repository `{}` for contract `{}`: {error}",
                    repository.id, contract.resource
                ),
            }
        })
    }

    fn repository_spec(self) -> Result<RepositorySpec, DataModelError> {
        match self {
            Self::EventsWaitlist => RepositorySpec::new(
                "events.waitlist",
                TableName::new("davenda.events_waitlist_entries")?,
                vec![
                    QueryField::new("waitlist_entry_id")?,
                    QueryField::new("event_id")?,
                    QueryField::new("slot_id")?,
                    QueryField::new("status")?,
                    QueryField::new("position")?,
                    QueryField::new("created_at")?,
                ],
            )?
            .with_sortable_field("created_at")?
            .with_default_sort(QuerySort::ascending("created_at")?)
            .with_filterable_field("event_id")?
            .with_filterable_field("slot_id"),
            Self::CmsPagesLive => Ok(
                RepositorySpec::new(
                    "cms.pages.live",
                    TableName::new("davenda.cms_pages")?,
                    vec![
                        QueryField::new("page_id")?,
                        QueryField::new("title")?,
                        QueryField::new("live_path")?,
                        QueryField::new("updated_at")?,
                    ],
                )?
                .with_locale_field("locale")?
                .with_publication_field("workflow_status", "published")?
                .with_filterable_field("slug")?
                .with_sortable_field("live_path")?
                .with_default_sort(QuerySort::ascending("live_path")?),
            ),
            Self::CommerceCatalogProducts => Ok(
                RepositorySpec::new(
                    "commerce.catalog.products",
                    TableName::new("davenda.catalog_products")?,
                    vec![
                        QueryField::new("product_id")?,
                        QueryField::new("product_title")?,
                        QueryField::new("product_slug")?,
                        QueryField::new("updated_at")?,
                    ],
                )?
                .with_locale_field("locale")?
                .with_publication_field("catalog_status", "active")?
                .with_filterable_field("collection_handle")?
                .with_sortable_field("product_title")?
                .with_default_sort(QuerySort::ascending("product_title")?),
            ),
        }
    }

    fn query_spec(self, context: &InvocationContext) -> Result<QuerySpec, DataModelError> {
        let locale = context.customer_app.locale.clone();
        let principal_id = context.principal.id.clone();
        match self {
            Self::EventsWaitlist => Ok(QuerySpec::new(
                PageRequest::new(0, 50)?,
                QueryContext {
                    locale: None,
                    principal_id,
                    publication_visibility: PublicationVisibility::IncludeDrafts,
                    cache_scope: QueryCacheScope::Uncacheable,
                },
            )),
            Self::CmsPagesLive => Ok(QuerySpec::new(
                PageRequest::new(0, 24)?,
                QueryContext {
                    locale,
                    principal_id: None,
                    publication_visibility: PublicationVisibility::PublishedOnly,
                    cache_scope: if context.customer_app.locale.is_some() {
                        QueryCacheScope::LocaleScoped
                    } else {
                        QueryCacheScope::Public
                    },
                },
            )),
            Self::CommerceCatalogProducts => Ok(QuerySpec::new(
                PageRequest::new(0, 24)?,
                QueryContext {
                    locale,
                    principal_id: None,
                    publication_visibility: PublicationVisibility::PublishedOnly,
                    cache_scope: if context.customer_app.locale.is_some() {
                        QueryCacheScope::LocaleScoped
                    } else {
                        QueryCacheScope::Public
                    },
                },
            )),
        }
    }
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
