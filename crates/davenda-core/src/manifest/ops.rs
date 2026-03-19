use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Csv,
    Json,
    Pdf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportSensitivity {
    Public,
    Internal,
    Restricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportDeliveryMode {
    PublicObjectStore,
    SignedUrl,
    InternalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportDefinition {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub format: ReportFormat,
    pub sensitivity: ReportSensitivity,
    pub delivery_mode: ReportDeliveryMode,
    pub export_prefix: String,
    pub retry_policy: RetryPolicy,
}

impl ReportDefinition {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        format: ReportFormat,
        sensitivity: ReportSensitivity,
        delivery_mode: ReportDeliveryMode,
        export_prefix: impl Into<String>,
        retry_policy: RetryPolicy,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description,
            required_capability,
            format,
            sensitivity,
            delivery_mode,
            export_prefix: export_prefix.into(),
            retry_policy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationKind {
    Publish,
    Unpublish,
    Reindex,
    Export,
    Cancel,
    CheckIn,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOperationScope {
    Cms,
    Commerce,
    Memberships,
    Events,
    Media,
    Search,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkOperationDefinition {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub required_capability: Capability,
    pub kind: BulkOperationKind,
    pub scope: BulkOperationScope,
    pub retry_policy: RetryPolicy,
    pub max_items: Option<usize>,
    pub requires_idempotency_key: bool,
}

impl BulkOperationDefinition {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Option<String>,
        required_capability: Capability,
        kind: BulkOperationKind,
        scope: BulkOperationScope,
        retry_policy: RetryPolicy,
        max_items: Option<usize>,
        requires_idempotency_key: bool,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description,
            required_capability,
            kind,
            scope,
            retry_policy,
            max_items,
            requires_idempotency_key,
        }
    }
}
