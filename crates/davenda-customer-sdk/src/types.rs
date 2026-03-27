use std::collections::BTreeMap;

pub type MetadataMap = BTreeMap<String, String>;
pub type Headers = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerPluginDescriptor {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub documentation_url: Option<String>,
}

impl CustomerPluginDescriptor {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            version: version.into(),
            documentation_url: None,
        }
    }

    pub fn with_documentation_url(mut self, documentation_url: impl Into<String>) -> Self {
        self.documentation_url = Some(documentation_url.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppContext {
    pub app_id: String,
    pub environment: String,
    pub locale: Option<String>,
}

impl CustomerAppContext {
    pub fn new(app_id: impl Into<String>, environment: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            environment: environment.into(),
            locale: None,
        }
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    Anonymous,
    User,
    ServiceAccount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalContext {
    pub kind: PrincipalKind,
    pub id: Option<String>,
}

impl PrincipalContext {
    pub fn anonymous() -> Self {
        Self {
            kind: PrincipalKind::Anonymous,
            id: None,
        }
    }

    pub fn user(id: impl Into<String>) -> Self {
        Self {
            kind: PrincipalKind::User,
            id: Some(id.into()),
        }
    }

    pub fn service_account(id: impl Into<String>) -> Self {
        Self {
            kind: PrincipalKind::ServiceAccount,
            id: Some(id.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,
    pub request_id: Option<String>,
}

impl TraceContext {
    pub fn new(trace_id: impl Into<String>) -> Self {
        Self {
            trace_id: trace_id.into(),
            request_id: None,
        }
    }

    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestContext {
    pub customer_app: CustomerAppContext,
    pub principal: PrincipalContext,
    pub trace: TraceContext,
}

impl RequestContext {
    pub fn new(
        customer_app: CustomerAppContext,
        principal: PrincipalContext,
        trace: TraceContext,
    ) -> Self {
        Self {
            customer_app,
            principal,
            trace,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoneyAmount {
    pub currency_code: String,
    pub minor_units: i64,
}

impl MoneyAmount {
    pub fn new(currency_code: impl Into<String>, minor_units: i64) -> Self {
        Self {
            currency_code: currency_code.into(),
            minor_units,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderLineDraft {
    pub sku: String,
    pub title: String,
    pub quantity: u32,
    pub unit_price: MoneyAmount,
    pub product_kind: String,
    pub collection_handle: Option<String>,
    pub entitlement_key: Option<String>,
    pub metadata: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDraft {
    pub order_id: String,
    pub currency_code: String,
    pub subtotal: MoneyAmount,
    pub total: MoneyAmount,
    pub lines: Vec<OrderLineDraft>,
    pub metadata: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderRejection {
    pub code: String,
    pub message: String,
}

impl OrderRejection {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderAdjustment {
    pub reason: String,
    pub metadata: MetadataMap,
}

impl OrderAdjustment {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
            metadata: MetadataMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn with_metadata_entries<I, K, V>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        for (key, value) in entries {
            self.metadata.insert(key.into(), value.into());
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderReviewDecision {
    Approved,
    Rejected(OrderRejection),
    Adjusted(OrderAdjustment),
}

impl OrderReviewDecision {
    pub const fn approved() -> Self {
        Self::Approved
    }

    pub fn rejected(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Rejected(OrderRejection::new(code, message))
    }

    pub fn adjusted(reason: impl Into<String>) -> Self {
        Self::Adjusted(OrderAdjustment::new(reason))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsPageDraft {
    pub page_id: String,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub body_html: String,
    pub locale: Option<String>,
    pub metadata: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmsPublishDecision {
    Allow,
    Reject { code: String, message: String },
}

impl CmsPublishDecision {
    pub fn reject(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Reject {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedWebhook {
    pub source: String,
    pub event: String,
    pub headers: Headers,
    pub content_type: Option<String>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookHandlingResult {
    Accepted { detail: Option<String> },
    Rejected { code: String, message: String },
}

impl WebhookHandlingResult {
    pub fn accepted(detail: Option<String>) -> Self {
        Self::Accepted { detail }
    }

    pub fn rejected(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Rejected {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRequest {
    pub queue: String,
    pub job_name: String,
    pub idempotency_key: Option<String>,
    pub payload_description: String,
    pub metadata: MetadataMap,
}

impl JobRequest {
    pub fn new(
        queue: impl Into<String>,
        job_name: impl Into<String>,
        payload_description: impl Into<String>,
    ) -> Self {
        Self {
            queue: queue.into(),
            job_name: job_name.into(),
            idempotency_key: None,
            payload_description: payload_description.into(),
            metadata: MetadataMap::new(),
        }
    }

    pub fn with_idempotency_key(mut self, idempotency_key: impl Into<String>) -> Self {
        self.idempotency_key = Some(idempotency_key.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobReceipt {
    pub queue: String,
    pub job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryQuery {
    pub repository: String,
    pub key: Option<String>,
    pub filters: MetadataMap,
}

impl RepositoryQuery {
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            key: None,
            filters: MetadataMap::new(),
        }
    }

    pub fn with_key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRecord {
    pub id: String,
    pub fields: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRecordSet {
    pub repository: String,
    pub records: Vec<RepositoryRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryWrite {
    pub repository: String,
    pub record_id: String,
    pub fields: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryWriteReceipt {
    pub repository: String,
    pub record_id: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCheckRequest {
    pub capability: String,
    pub object: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthCheckResult {
    pub allowed: bool,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplainRequest {
    pub capability: String,
    pub object: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplanation {
    pub summary: String,
    pub traces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub action: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub outcome: String,
    pub detail: Option<String>,
    pub metadata: MetadataMap,
}

impl AuditEntry {
    pub fn new(
        action: impl Into<String>,
        resource_kind: impl Into<String>,
        resource_id: impl Into<String>,
        outcome: impl Into<String>,
    ) -> Self {
        Self {
            action: action.into(),
            resource_kind: resource_kind.into(),
            resource_id: resource_id.into(),
            outcome: outcome.into(),
            detail: None,
            metadata: MetadataMap::new(),
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundHttpRequest {
    pub integration: String,
    pub method: String,
    pub url: String,
    pub headers: Headers,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundHttpResponse {
    pub status: u16,
    pub headers: Headers,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedAsset {
    pub logical_path: String,
    pub storage_class: String,
    pub public_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetWriteRequest {
    pub logical_path: String,
    pub storage_class: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
    pub metadata: MetadataMap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetWriteReceipt {
    pub logical_path: String,
    pub storage_path: String,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommerceProduct {
    pub sku: String,
    pub handle: String,
    pub title: String,
    pub current_price: MoneyAmount,
    pub collection_handle: Option<String>,
    pub metadata: MetadataMap,
}
