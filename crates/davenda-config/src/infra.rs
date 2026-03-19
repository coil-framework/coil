use serde::{Deserialize, Serialize};

use crate::SecretRef;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TlsConfig {
    pub mode: TlsMode,
    #[serde(default)]
    pub challenge: Option<AcmeChallenge>,
    #[serde(default)]
    pub provider: Option<TlsProvider>,
    #[serde(default)]
    pub account_secret: Option<SecretRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_driver")]
    pub driver: DatabaseDriver,
    #[serde(default = "default_database_url_secret")]
    pub url: Option<SecretRef>,
    #[serde(default = "default_database_schema")]
    pub schema: String,
    #[serde(default = "default_migrations_table")]
    pub migrations_table: String,
    #[serde(default = "default_min_database_connections")]
    pub min_connections: u16,
    #[serde(default = "default_max_database_connections")]
    pub max_connections: u16,
    #[serde(default = "default_statement_timeout_secs")]
    pub statement_timeout_secs: u64,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            driver: default_database_driver(),
            url: default_database_url_secret(),
            schema: default_database_schema(),
            migrations_table: default_migrations_table(),
            min_connections: default_min_database_connections(),
            max_connections: default_max_database_connections(),
            statement_timeout_secs: default_statement_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseDriver {
    Postgres,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsMode {
    External,
    Acme,
    CloudflareOrigin,
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AcmeChallenge {
    #[serde(rename = "http-01")]
    Http01,
    #[serde(rename = "tls-alpn-01")]
    TlsAlpn01,
    #[serde(rename = "dns-01")]
    Dns01,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsProvider {
    CloudflareDns,
    CloudflareOriginCa,
    ManualImport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageConfig {
    pub default_class: StorageClass,
    #[serde(default = "default_storage_deployment")]
    pub deployment: StorageDeployment,
    #[serde(default = "default_local_only_storage_mode")]
    pub local_only: LocalOnlyStorageMode,
    #[serde(default)]
    pub object_store: Option<ObjectStoreKind>,
    pub local_root: String,
    #[serde(default)]
    pub object_store_secret: Option<SecretRef>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageDeployment {
    Distributed,
    SingleNode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LocalOnlyStorageMode {
    Disabled,
    ExplicitSingleNode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StorageClass {
    PublicAsset,
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ObjectStoreKind {
    S3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheConfig {
    pub l1: CacheL1,
    #[serde(default)]
    pub l2: Option<DistributedCache>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CacheL1 {
    Moka,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DistributedCache {
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JobsConfig {
    pub backend: JobBackend,
    #[serde(default = "default_retry_limit")]
    pub retry_limit: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobBackend {
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservabilityConfig {
    pub metrics: bool,
    pub tracing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetsConfig {
    pub publish_manifest: bool,
    #[serde(default)]
    pub cdn_base_url: Option<String>,
}

fn default_retry_limit() -> u32 {
    5
}

fn default_database_driver() -> DatabaseDriver {
    DatabaseDriver::Postgres
}

fn default_database_url_secret() -> Option<SecretRef> {
    Some(SecretRef::Env {
        var: "DATABASE_URL".to_string(),
    })
}

fn default_database_schema() -> String {
    "public".to_string()
}

fn default_migrations_table() -> String {
    "_davenda_migrations".to_string()
}

fn default_min_database_connections() -> u16 {
    4
}

fn default_max_database_connections() -> u16 {
    32
}

fn default_statement_timeout_secs() -> u64 {
    30
}

fn default_storage_deployment() -> StorageDeployment {
    StorageDeployment::Distributed
}

fn default_local_only_storage_mode() -> LocalOnlyStorageMode {
    LocalOnlyStorageMode::Disabled
}
