use super::*;
use davenda_cache::DistributedCacheBackend;
use davenda_config::{
    DatabaseDriver, DistributedCache, JobBackend, ObjectStoreKind, SecretRef, SessionStore,
};
use davenda_storage::execution::ObjectStoreClientConfig;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretResolutionError {
    #[error("secret `{reference}` was not provided to the runtime")]
    MissingSecret { reference: String },
}

pub trait SecretResolver {
    fn resolve(&self, secret: &SecretRef) -> Result<String, SecretResolutionError>;
}

#[derive(Debug, Clone, Default)]
pub struct StaticSecretResolver {
    values: BTreeMap<String, String>,
}

impl StaticSecretResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_secret(
        mut self,
        secret: SecretRef,
        value: impl Into<String>,
    ) -> Result<Self, SecretResolutionError> {
        self.values.insert(secret.redacted(), value.into());
        Ok(self)
    }
}

impl SecretResolver for StaticSecretResolver {
    fn resolve(&self, secret: &SecretRef) -> Result<String, SecretResolutionError> {
        self.values.get(&secret.redacted()).cloned().ok_or_else(|| {
            SecretResolutionError::MissingSecret {
                reference: secret.redacted(),
            }
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseClientTarget {
    pub driver: DatabaseDriver,
    pub url: Option<String>,
    pub min_connections: u16,
    pub max_connections: u16,
    pub statement_timeout_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistributedCacheClientTarget {
    pub backend: DistributedCacheBackend,
    pub purpose: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobsClientTarget {
    pub backend: JobBackend,
    pub shared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStoreClientTarget {
    pub kind: SessionStoreBackendKind,
    pub shared: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreClientTarget {
    pub kind: ObjectStoreKind,
    pub endpoint_url: Option<String>,
    pub credential_reference: Option<String>,
    pub local_root: String,
}

impl ObjectStoreClientTarget {
    pub fn object_store_client_config(&self) -> Option<ObjectStoreClientConfig> {
        self.endpoint_url
            .as_ref()
            .map(|endpoint_url| ObjectStoreClientConfig::new(endpoint_url.clone()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedBackendClients {
    pub database: DatabaseClientTarget,
    pub distributed_cache: Option<DistributedCacheClientTarget>,
    pub jobs: JobsClientTarget,
    pub session_store: Option<SessionStoreClientTarget>,
    pub object_store: Option<ObjectStoreClientTarget>,
}

impl SharedBackendClients {
    pub fn from_config<R: SecretResolver>(
        config: &PlatformConfig,
        resolver: &R,
    ) -> Result<Self, SecretResolutionError> {
        let database = DatabaseClientTarget {
            driver: config.database.driver,
            url: config
                .database
                .url
                .as_ref()
                .map(|secret| resolver.resolve(secret))
                .transpose()?,
            min_connections: config.database.min_connections,
            max_connections: config.database.max_connections,
            statement_timeout_secs: config.database.statement_timeout_secs,
        };
        let distributed_cache = config.cache.l2.map(|backend| DistributedCacheClientTarget {
            backend: distributed_cache_backend(backend),
            purpose: "cache-and-coordination",
        });
        let jobs = JobsClientTarget {
            backend: config.jobs.backend,
            shared: true,
        };
        let session_store = match config.http.session.store {
            SessionStore::Memory => None,
            SessionStore::Database => Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Database,
                shared: true,
            }),
            SessionStore::Redis => Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Redis,
                shared: true,
            }),
            SessionStore::Valkey => Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Valkey,
                shared: true,
            }),
        };
        let object_store_credentials = config
            .storage
            .object_store_secret
            .as_ref()
            .map(|secret| resolver.resolve(secret))
            .transpose()?;
        let object_store = config
            .storage
            .object_store
            .map(|kind| ObjectStoreClientTarget {
                kind,
                endpoint_url: object_store_credentials.clone(),
                credential_reference: object_store_credentials.clone(),
                local_root: config.storage.local_root.clone(),
            });

        Ok(Self {
            database,
            distributed_cache,
            jobs,
            session_store,
            object_store,
        })
    }
}

impl fmt::Display for SharedBackendClients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "db={:?} cache={:?} jobs={:?} sessions={:?} object_store={:?}",
            self.database.driver,
            self.distributed_cache.as_ref().map(|cache| cache.backend),
            self.jobs.backend,
            self.session_store.as_ref().map(|store| store.kind),
            self.object_store.as_ref().map(|store| store.kind)
        )
    }
}

fn distributed_cache_backend(cache: DistributedCache) -> DistributedCacheBackend {
    match cache {
        DistributedCache::Redis => DistributedCacheBackend::Redis,
        DistributedCache::Valkey => DistributedCacheBackend::Valkey,
    }
}
