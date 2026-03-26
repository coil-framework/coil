use super::*;
use davenda_assets::ActiveAssetManifest;
use davenda_storage::execution::ObjectStoreClientConfig;
use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use url::Url;

mod execution;
mod shared_state;
#[cfg(test)]
mod testing;

pub(crate) use shared_state::shared_state_root;
#[cfg(test)]
pub(crate) use testing::{shared_cache_runtime_for_test, shared_jobs_runtime_for_test};

#[derive(Clone)]
pub(crate) struct SharedJobsRuntimeHandle {
    namespace: String,
    runtime: Arc<OnceLock<Result<Arc<dyn davenda_jobs::JobsCoordinationRuntime>, String>>>,
}

impl SharedJobsRuntimeHandle {
    pub(crate) fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            runtime: Arc::new(OnceLock::new()),
        }
    }

    pub(crate) fn get_or_init(
        &self,
        runtime: &JobsRuntimeServices,
    ) -> Result<Arc<dyn davenda_jobs::JobsCoordinationRuntime>, RuntimeJobsError> {
        let namespace = self.namespace.clone();
        self.runtime
            .get_or_init(|| shared_jobs_runtime(runtime, namespace.clone()))
            .clone()
            .map_err(|error| {
                RuntimeJobsError::Jobs(JobsModelError::LiveSharedBackendRequiresExplicitRuntime {
                    backend: runtime.backend,
                    namespace: error,
                })
            })
    }
}

impl fmt::Debug for SharedJobsRuntimeHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedJobsRuntimeHandle")
            .field("namespace", &self.namespace)
            .field("initialized", &self.runtime.get().is_some())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub auth_package: AuthModelPackageSelection,
    pub approved_outbound_http_endpoints: BTreeMap<String, Url>,
    pub shared_backend_scope: String,
    pub shared_state_root: PathBuf,
    pub cache_topology: CacheTopology,
    pub cache_planner: CachePlanner,
    pub i18n: I18nRuntimeServices,
    pub seo: SeoRuntimeServices,
    pub browser: BrowserSecurityServices,
    pub cli: CliRuntimeServices,
    pub data: DataRuntimeServices,
    pub jobs: JobsRuntimeServices,
    pub observability: ObservabilityRuntimeServices,
    pub http: HttpRuntimePlan,
    pub handlers: BTreeMap<String, HandlerDefinition>,
    pub storage_planner: StoragePlanner,
    pub storefront_catalog: StorefrontCatalog,
    pub theme_asset_manifest: Option<ActiveAssetManifest>,
    pub template: TemplateRuntimeServices,
    pub tls: TlsRuntimeServices,
    pub wasm: WasmRuntimeServices,
    pub services: Vec<ServiceDescriptor>,
    pub modules: Vec<ModuleManifest>,
    pub install_migrations: MigrationPlan,
    pub extension_registry: ExtensionRegistry,
    pub registered_extension_slots: Vec<RegisteredExtensionSlot>,
    pub installed_extensions: Vec<InstalledExtensionSummary>,
    pub(crate) shared_jobs_runtime: SharedJobsRuntimeHandle,
    pub module_jobs: Vec<RegisteredModuleJob>,
    pub module_event_subscriptions: Vec<RegisteredEventSubscription>,
    pub module_data_repositories: Vec<RegisteredDataRepository>,
    pub module_search_contributions: Vec<RegisteredSearchContribution>,
    pub module_report_definitions: Vec<RegisteredReportDefinition>,
    pub module_bulk_operations: Vec<RegisteredBulkOperation>,
    pub registered_runtime_jobs: Vec<RuntimeJobDefinition>,
    pub registered_runtime_event_subscriptions: Vec<RuntimeEventSubscriptionDefinition>,
    pub jobs_domain: JobsDomain,
    pub ops_catalog: OpsCatalog,
}

#[derive(Debug, Clone)]
pub(crate) enum MetadataAuditBackendSelection {
    LocalSqlite {
        root: std::path::PathBuf,
        namespace: String,
    },
    SharedPostgres {
        runtime: davenda_data::DataRuntime,
    },
}

impl RuntimePlan {
    pub fn auth_package(&self) -> &dyn AuthModelPackage {
        self.auth_package.package()
    }

    pub(crate) fn metadata_audit_backend_selection(&self) -> MetadataAuditBackendSelection {
        match self.config.storage.deployment {
            davenda_config::StorageDeployment::Distributed => {
                MetadataAuditBackendSelection::SharedPostgres {
                    runtime: self.data.clone(),
                }
            }
            davenda_config::StorageDeployment::SingleNode => {
                MetadataAuditBackendSelection::LocalSqlite {
                    root: std::path::PathBuf::from(&self.config.storage.local_root),
                    namespace: self.shared_backend_namespace(),
                }
            }
        }
    }

    pub fn approved_outbound_http_endpoints(&self) -> &BTreeMap<String, Url> {
        &self.approved_outbound_http_endpoints
    }

    pub fn tenant_id(&self) -> i64 {
        self.config.auth.tenant_id
    }

    pub fn jobs_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<JobsHost, RuntimeJobsError> {
        let scheduler_node_id =
            validate_runtime_identifier("scheduler_node_id", scheduler_node_id.into())?;
        let namespace = self.shared_backend_namespace();
        let shared_runtime = self.shared_jobs_runtime.get_or_init(&self.jobs)?;
        Ok(JobsHost::new(
            self.config.app.name.clone(),
            scheduler_node_id,
            self.jobs.clone(),
            self.jobs.describe().clone(),
            self.registered_runtime_jobs.clone(),
            self.registered_runtime_event_subscriptions.clone(),
            self.jobs_domain.clone(),
            shared_runtime,
            namespace,
        ))
    }

    pub fn ops_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<OpsHost, RuntimeOpsError> {
        Ok(OpsHost::new(
            OpsPlanner::new(self.jobs.clone(), self.ops_catalog.clone())?,
            self.jobs_host(scheduler_node_id)?,
        ))
    }

    pub fn search_host(
        &self,
        scheduler_node_id: impl Into<String>,
    ) -> Result<SearchHost, RuntimeSearchError> {
        Ok(SearchHost::new(
            self.ops_catalog.search.clone(),
            self.ops_host(scheduler_node_id)?,
        ))
    }

    pub fn cache_host(&self) -> Result<CacheHost, RuntimeCacheError> {
        let namespace = self.cache_namespace()?;
        let shared_namespace = self.shared_backend_namespace();
        if self.cache_planner.topology().supports_shared_invalidation() {
            #[cfg(test)]
            {
                let backend = match self
                    .cache_planner
                    .topology()
                    .l2()
                    .expect("shared cache runtime requires distributed l2")
                {
                    davenda_cache::DistributedCacheBackend::Redis => {
                        davenda_cache::CacheBackendKind::Redis
                    }
                    davenda_cache::DistributedCacheBackend::Valkey => {
                        davenda_cache::CacheBackendKind::Valkey
                    }
                };
                let runtime = shared_cache_runtime_for_test(backend, shared_namespace.clone());
                return Ok(CacheHost::new(
                    self.config.app.name.clone(),
                    namespace,
                    self.cache_planner,
                    Some(runtime),
                    shared_namespace,
                ));
            }

            #[cfg(not(test))]
            {
                let backend = match self
                    .cache_planner
                    .topology()
                    .l2()
                    .expect("shared cache runtime requires distributed l2")
                {
                    davenda_cache::DistributedCacheBackend::Redis => {
                        davenda_cache::CacheBackendKind::Redis
                    }
                    davenda_cache::DistributedCacheBackend::Valkey => {
                        davenda_cache::CacheBackendKind::Valkey
                    }
                };
                let runtime = shared_cache_runtime(backend, shared_namespace.clone());
                return Ok(CacheHost::new(
                    self.config.app.name.clone(),
                    namespace,
                    self.cache_planner,
                    Some(runtime),
                    shared_namespace,
                ));
            }
        }

        Ok(CacheHost::new(
            self.config.app.name.clone(),
            namespace,
            self.cache_planner,
            None,
            shared_namespace,
        ))
    }

    #[cfg(test)]
    pub fn browser_host(&self) -> Result<BrowserHost, BrowserHostBuildError> {
        BrowserHost::new_with_scope(
            self.config.app.name.clone(),
            self.browser.clone(),
            self.shared_backend_scope.clone(),
        )
    }

    pub fn tls_host(&self) -> Result<TlsHost, RuntimeTlsError> {
        self.tls_host_with_secret_resolver(&crate::server::EnvironmentSecretResolver)
    }

    pub fn tls_host_with_secret_resolver<R: crate::server::SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<TlsHost, RuntimeTlsError> {
        let account_secret = self
            .config
            .tls
            .account_secret
            .as_ref()
            .map(|secret| resolver.resolve(secret))
            .transpose()?;
        TlsHost::new(
            self.config.app.name.clone(),
            self.tls.clone(),
            self.data.clone(),
            self.shared_backend_scope.clone(),
            account_secret,
        )
    }

    pub fn tls_validation_host_with_secret_resolver<R: crate::server::SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<TlsHost, RuntimeTlsError> {
        let account_secret = self
            .config
            .tls
            .account_secret
            .as_ref()
            .map(|secret| resolver.resolve(secret))
            .transpose()?;
        TlsHost::new_for_validation(
            self.config.app.name.clone(),
            self.tls.clone(),
            self.shared_backend_scope.clone(),
            account_secret,
        )
    }

    pub fn storage_host(&self) -> StorageHost {
        self.storage_host_with_object_store(None)
    }

    pub fn storage_host_with_object_store(
        &self,
        object_store: Option<ObjectStoreClientConfig>,
    ) -> StorageHost {
        StorageHost::new(
            self.config.app.name.clone(),
            self.storage_planner.clone(),
            self.config.assets.cdn_base_url.clone(),
            object_store,
        )
    }

    pub fn wasm_host(&self) -> WasmHost {
        WasmHost::new(
            self.clone(),
            self.config.app.name.clone(),
            self.wasm.clone(),
            self.extension_registry.clone(),
            self.config.i18n.default_locale.clone(),
            self.registered_runtime_jobs.clone(),
        )
    }

    pub fn wasm_host_with_secret_resolver<R: SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<WasmHost, RuntimeServerError> {
        let wasm_secrets = self.wasm_secret_values(resolver)?;
        let storage_host =
            self.storage_host_with_object_store(self.object_store_client_config(resolver)?);
        Ok(WasmHost::with_host_services(
            self.clone(),
            self.config.app.name.clone(),
            self.wasm.clone(),
            self.extension_registry.clone(),
            self.config.i18n.default_locale.clone(),
            self.registered_runtime_jobs.clone(),
            RuntimeWasmHostServices::with_runtime_secrets(self.clone(), storage_host, wasm_secrets),
        ))
    }

    pub fn wasm_secret_values<R: SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<BTreeMap<String, String>, RuntimeServerError> {
        self.config
            .wasm
            .secret_bindings
            .iter()
            .map(|(name, secret)| {
                resolver
                    .resolve(secret)
                    .map(|value| (name.clone(), value))
                    .map_err(|error| RuntimeServerError::Secret(error))
            })
            .collect()
    }

    pub fn shared_backend_clients<R: SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<SharedBackendClients, RuntimeServerError> {
        Ok(SharedBackendClients::from_config(&self.config, resolver)?)
    }

    pub fn object_store_client_config<R: SecretResolver>(
        &self,
        resolver: &R,
    ) -> Result<Option<ObjectStoreClientConfig>, RuntimeServerError> {
        Ok(SharedBackendClients::object_store_client_config(
            &self.config,
            resolver,
        )?)
    }

    pub fn server_host<R: SecretResolver>(
        &self,
        resolver: &R,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
    ) -> Result<HttpServerHost, RuntimeServerError> {
        if self.browser.sessions.store == davenda_core::SessionStoreTopology::Memory {
            return Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost.into());
        }

        let wasm_secrets = self.wasm_secret_values(resolver)?;
        let payment_webhook_secret =
            crate::server::resolve_commerce_payment_webhook_secret(&self.config, resolver)?;
        HttpServerHost::new(
            self.clone(),
            self.shared_backend_clients(resolver)?,
            wasm_secrets,
            payment_webhook_secret,
            cookie_secret.to_vec(),
            csrf_secret.to_vec(),
        )
    }

    #[cfg(test)]
    pub(crate) fn server_host_with_checkout_client<R: SecretResolver>(
        &self,
        resolver: &R,
        cookie_secret: &[u8],
        csrf_secret: &[u8],
        hosted_checkout_client: std::sync::Arc<dyn crate::server::HostedCheckoutClient>,
    ) -> Result<HttpServerHost, RuntimeServerError> {
        if self.browser.sessions.store == davenda_core::SessionStoreTopology::Memory {
            return Err(BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost.into());
        }

        let wasm_secrets = self.wasm_secret_values(resolver)?;
        let payment_webhook_secret =
            crate::server::resolve_commerce_payment_webhook_secret(&self.config, resolver)?;
        HttpServerHost::new_with_checkout_client(
            self.clone(),
            self.shared_backend_clients(resolver)?,
            wasm_secrets,
            payment_webhook_secret,
            cookie_secret.to_vec(),
            csrf_secret.to_vec(),
            hosted_checkout_client,
        )
    }

    pub(crate) fn cache_namespace(&self) -> Result<CacheNamespace, CacheModelError> {
        CacheNamespace::new(format!("customer-app:{}", self.config.app.name))
    }

    pub(crate) fn shared_backend_namespace(&self) -> String {
        format!(
            "customer-app:{}:{}",
            self.config.app.name, self.shared_backend_scope
        )
    }

    pub(crate) fn shared_state_root(&self) -> &PathBuf {
        &self.shared_state_root
    }
}

#[cfg(test)]
fn shared_jobs_runtime(
    runtime: &JobsRuntimeServices,
    namespace: String,
) -> Result<Arc<dyn davenda_jobs::JobsCoordinationRuntime>, String> {
    Ok(crate::plan::shared_jobs_runtime_for_test(
        runtime, namespace,
    ))
}

#[cfg(not(test))]
fn shared_jobs_runtime(
    runtime: &JobsRuntimeServices,
    namespace: String,
) -> Result<Arc<dyn davenda_jobs::JobsCoordinationRuntime>, String> {
    davenda_jobs::JobsBackendAdapter::live_shared_runtime(runtime, namespace, PathBuf::new())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
fn shared_cache_runtime(
    backend: davenda_cache::CacheBackendKind,
    namespace: String,
) -> Arc<dyn davenda_cache::DistributedCacheRuntime> {
    crate::plan::shared_cache_runtime_for_test(backend, namespace)
}

#[cfg(not(test))]
fn shared_cache_runtime(
    backend: davenda_cache::CacheBackendKind,
    namespace: String,
) -> Arc<dyn davenda_cache::DistributedCacheRuntime> {
    davenda_cache::DistributedCacheClient::live_shared_runtime(backend, namespace, PathBuf::new())
}
