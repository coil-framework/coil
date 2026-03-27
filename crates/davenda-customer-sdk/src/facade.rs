use crate::{
    AssetWriteReceipt, AssetWriteRequest, AuditEntry, AuthCheckRequest, AuthCheckResult,
    AuthExplainRequest, AuthExplanation, BackendError, CommerceProduct, JobReceipt, JobRequest,
    ManagedAsset, OutboundHttpRequest, OutboundHttpResponse, RepositoryQuery, RepositoryRecordSet,
    RepositoryWrite, RepositoryWriteReceipt,
};

pub trait CommerceFacade: Send + Sync {
    fn product(&self, sku: &str) -> Result<Option<CommerceProduct>, BackendError>;

    fn add_order_note(&self, order_id: &str, note: &str) -> Result<(), BackendError>;
}

pub trait JobsFacade: Send + Sync {
    fn enqueue(&self, request: JobRequest) -> Result<JobReceipt, BackendError>;
}

pub trait RepositoryFacade: Send + Sync {
    fn read(&self, query: &RepositoryQuery) -> Result<RepositoryRecordSet, BackendError>;

    fn write(&self, change: RepositoryWrite) -> Result<RepositoryWriteReceipt, BackendError>;
}

pub trait AuthFacade: Send + Sync {
    fn check_capability(&self, request: &AuthCheckRequest)
    -> Result<AuthCheckResult, BackendError>;

    fn explain_denial(&self, request: &AuthExplainRequest)
    -> Result<AuthExplanation, BackendError>;
}

pub trait AuditFacade: Send + Sync {
    fn record(&self, entry: AuditEntry) -> Result<(), BackendError>;
}

pub trait OutboundHttpFacade: Send + Sync {
    fn send(&self, request: OutboundHttpRequest) -> Result<OutboundHttpResponse, BackendError>;
}

pub trait AssetsFacade: Send + Sync {
    fn publish(&self, request: AssetWriteRequest) -> Result<AssetWriteReceipt, BackendError>;

    fn inspect(&self, logical_path: &str) -> Result<Option<ManagedAsset>, BackendError>;
}
