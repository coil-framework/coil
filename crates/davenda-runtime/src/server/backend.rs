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
    #[error("secret `{reference}` uses a source that is not available in this runtime context")]
    UnsupportedSecretSource { reference: String },
    #[error("object-store backend `{kind:?}` requires an object-store secret")]
    MissingObjectStoreSecret { kind: ObjectStoreKind },
    #[error("object-store secret `{reference}` is invalid: {message}")]
    InvalidObjectStoreConfig { reference: String, message: String },
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

#[derive(Debug, Clone, Default)]
pub struct EnvironmentSecretResolver;

impl SecretResolver for EnvironmentSecretResolver {
    fn resolve(&self, secret: &SecretRef) -> Result<String, SecretResolutionError> {
        match secret {
            SecretRef::Env { var } => {
                std::env::var(var).map_err(|_| SecretResolutionError::MissingSecret {
                    reference: secret.redacted(),
                })
            }
            SecretRef::SecretManager { .. } => {
                Err(SecretResolutionError::UnsupportedSecretSource {
                    reference: secret.redacted(),
                })
            }
        }
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
    pub bucket: Option<String>,
    pub region: Option<String>,
    pub credential_reference: Option<String>,
    pub signed_url_ttl_secs: Option<u64>,
    pub local_root: String,
    config: ObjectStoreClientConfig,
}

impl ObjectStoreClientTarget {
    pub fn object_store_client_config(&self) -> Option<ObjectStoreClientConfig> {
        Some(self.config.clone())
    }

    fn new(
        kind: ObjectStoreKind,
        config: ObjectStoreClientConfig,
        credential_reference: Option<String>,
        local_root: String,
    ) -> Self {
        let endpoint_url = config.endpoint_url.clone();
        let bucket = Some(config.bucket.clone());
        let region = Some(config.region.clone());
        let signed_url_ttl_secs = Some(config.signed_url_ttl_secs);
        Self {
            kind,
            endpoint_url,
            bucket,
            region,
            credential_reference,
            signed_url_ttl_secs,
            local_root,
            config,
        }
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
    pub fn object_store_client_config<R: SecretResolver>(
        config: &PlatformConfig,
        resolver: &R,
    ) -> Result<Option<ObjectStoreClientConfig>, SecretResolutionError> {
        resolve_object_store_client_config(config, resolver)
    }

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
        let object_store = config
            .storage
            .object_store
            .map(|kind| {
                let credential_reference = config
                    .storage
                    .object_store_secret
                    .as_ref()
                    .map(SecretRef::redacted);
                let client_config = Self::object_store_client_config(config, resolver)?
                    .expect("object-store config should be present when backend is enabled");
                Ok(ObjectStoreClientTarget::new(
                    kind,
                    client_config,
                    credential_reference,
                    config.storage.local_root.clone(),
                ))
            })
            .transpose()?;

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

fn resolve_object_store_client_config<R: SecretResolver>(
    config: &PlatformConfig,
    resolver: &R,
) -> Result<Option<ObjectStoreClientConfig>, SecretResolutionError> {
    let Some(kind) = config.storage.object_store else {
        return Ok(None);
    };
    let secret = config
        .storage
        .object_store_secret
        .as_ref()
        .ok_or(SecretResolutionError::MissingObjectStoreSecret { kind })?;
    let value = resolver.resolve(secret)?;
    let config = ObjectStoreClientConfig::from_secret_value(&value).map_err(|error| {
        SecretResolutionError::InvalidObjectStoreConfig {
            reference: secret.redacted(),
            message: error.to_string(),
        }
    })?;
    Ok(Some(config))
}

fn distributed_cache_backend(cache: DistributedCache) -> DistributedCacheBackend {
    match cache {
        DistributedCache::Redis => DistributedCacheBackend::Redis,
        DistributedCache::Valkey => DistributedCacheBackend::Valkey,
    }
}
