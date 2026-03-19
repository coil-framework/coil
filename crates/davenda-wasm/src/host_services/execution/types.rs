use crate::grants::MetadataGrant;
use crate::host_api::HostServiceCall;

use super::auth::AuthServiceExecution;
use super::cache::CacheIntentExecution;
use super::data::DataServiceExecution;
use super::render::RenderServiceExecution;
use super::storage::StorageServiceExecution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostServiceExecution {
    pub call: HostServiceCall,
    pub result: HostServiceResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostServiceResult {
    Auth(AuthServiceExecution),
    Data(DataServiceExecution),
    Storage(StorageServiceExecution),
    Render(RenderServiceExecution),
    CacheIntent(CacheIntentExecution),
    Network(NetworkExecution),
    Secret(SecretExecution),
    Job(JobExecution),
    Metadata(MetadataExecution),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkExecution {
    pub integration: String,
    pub endpoint: String,
    pub status: u16,
    pub response_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretExecution {
    pub secret: String,
    pub source: String,
    pub value_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExecution {
    pub queue: String,
    pub job_id: String,
    pub enqueued_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataExecution {
    pub kind: MetadataGrant,
    pub recorded: bool,
    pub journal_entries: usize,
}
