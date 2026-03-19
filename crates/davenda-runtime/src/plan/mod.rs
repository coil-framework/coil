use super::*;
use url::Url;

mod execution;
mod live;
#[cfg(test)]
mod testing;

#[cfg(test)]
pub(crate) use testing::{shared_cache_runtime_for_test, shared_jobs_runtime_for_test};

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub auth_package: AuthModelPackageSelection,
    pub approved_outbound_http_endpoints: BTreeMap<String, Url>,
    pub shared_backend_scope: String,
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
    pub template: TemplateRuntimeServices,
    pub tls: TlsRuntimeServices,
    pub wasm: WasmRuntimeServices,
    pub services: Vec<ServiceDescriptor>,
    pub modules: Vec<ModuleManifest>,
    pub install_migrations: MigrationPlan,
    pub extension_registry: ExtensionRegistry,
    pub registered_extension_slots: Vec<RegisteredExtensionSlot>,
    pub installed_extensions: Vec<InstalledExtensionSummary>,
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
        #[cfg(test)]
        let shared_runtime = shared_jobs_runtime_for_test(&self.jobs, namespace.clone());
        #[cfg(not(test))]
        // Live builds never fall back to local/shared-volume jobs state.
        let shared_runtime = live::live_rejection_jobs_runtime(&self.jobs, namespace.clone());
        #[cfg(not(test))]
        if !shared_runtime.is_shared_backend() {
            return Err(RuntimeJobsError::LiveSharedRuntimeRequiresExplicitBackend {
                backend: self.jobs.backend,
            });
        }

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
        let shared_runtime = if self.cache_planner.topology().supports_shared_invalidation() {
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
            #[cfg(test)]
            let runtime = shared_cache_runtime_for_test(backend, shared_namespace.clone());
            #[cfg(not(test))]
            // Live builds never fall back to local/shared-volume cache state.
            let runtime = live::live_rejection_cache_runtime(backend, shared_namespace.clone());
            #[cfg(not(test))]
            if !runtime.is_shared_backend() {
                return Err(
                    RuntimeCacheError::LiveSharedRuntimeRequiresExplicitBackend { kind: backend },
                );
            }
            Some(runtime)
        } else {
            None
        };
        Ok(CacheHost::new(
            self.config.app.name.clone(),
            namespace,
            self.cache_planner,
            shared_runtime,
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
        TlsHost::new(
            self.config.app.name.clone(),
            self.tls.clone(),
            self.data.clone(),
            self.shared_backend_scope.clone(),
        )
    }

    pub fn storage_host(&self) -> StorageHost {
        StorageHost::new(
            self.config.app.name.clone(),
            self.storage_planner.clone(),
            self.config.assets.cdn_base_url.clone(),
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
        HttpServerHost::new(
            self.clone(),
            self.shared_backend_clients(resolver)?,
            wasm_secrets,
            cookie_secret.to_vec(),
            csrf_secret.to_vec(),
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
}
