use crate::{BackendError, BackendErrorKind};
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
    pub site_id: Option<String>,
    pub locale: Option<String>,
}

impl CustomerAppContext {
    pub fn new(app_id: impl Into<String>, environment: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            environment: environment.into(),
            site_id: None,
            locale: None,
        }
    }

    pub fn with_site_id(mut self, site_id: impl Into<String>) -> Self {
        self.site_id = Some(site_id.into());
        self
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

    pub fn with_filter(mut self, field: impl Into<String>, value: impl Into<String>) -> Self {
        self.filters.insert(field.into(), value.into());
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
pub struct CmsPageRecord {
    pub page_id: String,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body_html: String,
    pub status: String,
    pub live_path: Option<String>,
}

impl CmsPageRecord {
    pub const REPOSITORY: &'static str = "cms.pages";

    pub fn from_repository_record(record: &RepositoryRecord) -> Result<Self, BackendError> {
        Ok(Self {
            page_id: record.id.clone(),
            title: required_repository_field(record, "title")?,
            slug: required_repository_field(record, "slug")?,
            summary: required_repository_field(record, "summary")?,
            body_html: required_repository_field(record, "body_html")?,
            status: required_repository_field(record, "status")?,
            live_path: optional_repository_field(record, "live_path"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsPageUpdate {
    pub page_id: String,
    pub title: String,
    pub slug: String,
    pub summary: String,
    pub body_html: String,
}

impl CmsPageUpdate {
    pub fn new(
        page_id: impl Into<String>,
        title: impl Into<String>,
        slug: impl Into<String>,
        summary: impl Into<String>,
        body_html: impl Into<String>,
    ) -> Self {
        Self {
            page_id: page_id.into(),
            title: title.into(),
            slug: slug.into(),
            summary: summary.into(),
            body_html: body_html.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsNavigationRecord {
    pub record_id: usize,
    pub label: String,
    pub href: String,
}

impl CmsNavigationRecord {
    pub const REPOSITORY: &'static str = "cms.navigation";

    pub fn from_repository_record(record: &RepositoryRecord) -> Result<Self, BackendError> {
        Ok(Self {
            record_id: record.id.parse::<usize>().map_err(|_| {
                BackendError::new(
                    BackendErrorKind::Conflict,
                    "repository.record.invalid_navigation_id",
                    format!(
                        "Repository record `{}` was not a valid CMS navigation record id.",
                        record.id
                    ),
                )
            })?,
            label: required_repository_field(record, "label")?,
            href: required_repository_field(record, "href")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsNavigationUpdate {
    pub record_id: usize,
    pub label: String,
    pub href: String,
}

impl CmsNavigationUpdate {
    pub fn new(record_id: usize, label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            record_id,
            label: label.into(),
            href: href.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsNavigationAppend {
    pub label: String,
    pub href: String,
}

impl CmsNavigationAppend {
    pub fn new(label: impl Into<String>, href: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            href: href.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsRedirectRecord {
    pub record_id: usize,
    pub from: String,
    pub to: String,
    pub permanent: bool,
}

impl CmsRedirectRecord {
    pub const REPOSITORY: &'static str = "cms.redirects";

    pub fn from_repository_record(record: &RepositoryRecord) -> Result<Self, BackendError> {
        Ok(Self {
            record_id: record.id.parse::<usize>().map_err(|_| {
                BackendError::new(
                    BackendErrorKind::Conflict,
                    "repository.record.invalid_redirect_id",
                    format!(
                        "Repository record `{}` was not a valid CMS redirect record id.",
                        record.id
                    ),
                )
            })?,
            from: required_repository_field(record, "from")?,
            to: required_repository_field(record, "to")?,
            permanent: required_repository_bool_field(record, "permanent")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsRedirectUpdate {
    pub record_id: usize,
    pub from: String,
    pub to: String,
    pub permanent: bool,
}

impl CmsRedirectUpdate {
    pub fn new(
        record_id: usize,
        from: impl Into<String>,
        to: impl Into<String>,
        permanent: bool,
    ) -> Self {
        Self {
            record_id,
            from: from.into(),
            to: to.into(),
            permanent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsRedirectAppend {
    pub from: String,
    pub to: String,
    pub permanent: bool,
}

impl CmsRedirectAppend {
    pub fn new(from: impl Into<String>, to: impl Into<String>, permanent: bool) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            permanent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommerceCatalogProductRecord {
    pub handle: String,
    pub sku: String,
    pub title: String,
    pub summary: String,
    pub price_minor: i64,
    pub currency: String,
    pub collection_handle: String,
    pub is_visible: bool,
    pub product_kind: String,
    pub entitlement_key: Option<String>,
}

impl CommerceCatalogProductRecord {
    pub const REPOSITORY: &'static str = "commerce.catalog.products";

    pub fn from_repository_record(record: &RepositoryRecord) -> Result<Self, BackendError> {
        Ok(Self {
            handle: required_repository_field(record, "handle")?,
            sku: required_repository_field(record, "sku")?,
            title: required_repository_field(record, "title")?,
            summary: required_repository_field(record, "summary")?,
            price_minor: required_repository_i64_field(record, "price_minor")?,
            currency: required_repository_field(record, "currency")?,
            collection_handle: required_repository_field(record, "collection_handle")?,
            is_visible: required_repository_bool_field(record, "is_visible")?,
            product_kind: required_repository_field(record, "product_kind")?,
            entitlement_key: optional_repository_field(record, "entitlement_key"),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommerceCatalogProductUpdate {
    pub handle: String,
    pub title: String,
    pub summary: String,
    pub price_minor: i64,
    pub collection_handle: String,
    pub is_visible: bool,
}

impl CommerceCatalogProductUpdate {
    pub fn new(
        handle: impl Into<String>,
        title: impl Into<String>,
        summary: impl Into<String>,
        price_minor: i64,
        collection_handle: impl Into<String>,
        is_visible: bool,
    ) -> Self {
        Self {
            handle: handle.into(),
            title: title.into(),
            summary: summary.into(),
            price_minor,
            collection_handle: collection_handle.into(),
            is_visible,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommerceCatalogCollectionRecord {
    pub handle: String,
    pub title: String,
    pub label: String,
    pub summary: String,
    pub is_visible: bool,
}

impl CommerceCatalogCollectionRecord {
    pub const REPOSITORY: &'static str = "commerce.catalog.collections";

    pub fn from_repository_record(record: &RepositoryRecord) -> Result<Self, BackendError> {
        Ok(Self {
            handle: required_repository_field(record, "handle")?,
            title: required_repository_field(record, "title")?,
            label: required_repository_field(record, "label")?,
            summary: required_repository_field(record, "summary")?,
            is_visible: required_repository_bool_field(record, "is_visible")?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommerceCatalogCollectionUpdate {
    pub handle: String,
    pub title: String,
    pub label: String,
    pub summary: String,
    pub is_visible: bool,
}

impl CommerceCatalogCollectionUpdate {
    pub fn new(
        handle: impl Into<String>,
        title: impl Into<String>,
        label: impl Into<String>,
        summary: impl Into<String>,
        is_visible: bool,
    ) -> Self {
        Self {
            handle: handle.into(),
            title: title.into(),
            label: label.into(),
            summary: summary.into(),
            is_visible,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommerceOrderRecord {
    pub order_id: String,
    pub status: String,
    pub payment_status: String,
    pub payment_reference: Option<String>,
    pub payment_method: Option<String>,
    pub checkout_email: Option<String>,
    pub principal_id: Option<String>,
    pub currency: String,
    pub total_minor: i64,
    pub line_count: usize,
}

impl CommerceOrderRecord {
    pub const REPOSITORY: &'static str = "commerce.orders";

    pub fn from_repository_record(record: &RepositoryRecord) -> Result<Self, BackendError> {
        Ok(Self {
            order_id: record.id.clone(),
            status: required_repository_field(record, "status")?,
            payment_status: required_repository_field(record, "payment_status")?,
            payment_reference: optional_repository_field(record, "payment_reference"),
            payment_method: optional_repository_field(record, "payment_method"),
            checkout_email: optional_repository_field(record, "checkout_email"),
            principal_id: optional_repository_field(record, "principal_id"),
            currency: required_repository_field(record, "currency")?,
            total_minor: required_repository_i64_field(record, "total_minor")?,
            line_count: required_repository_usize_field(record, "line_count")?,
        })
    }
}

fn required_repository_field(
    record: &RepositoryRecord,
    field: &str,
) -> Result<String, BackendError> {
    record.fields.get(field).cloned().ok_or_else(|| {
        BackendError::new(
            BackendErrorKind::Conflict,
            "repository.record.missing_field",
            format!(
                "Repository record `{}` did not expose required field `{field}`.",
                record.id
            ),
        )
    })
}

fn optional_repository_field(record: &RepositoryRecord, field: &str) -> Option<String> {
    record
        .fields
        .get(field)
        .cloned()
        .and_then(|value| (!value.trim().is_empty()).then_some(value))
}

fn required_repository_i64_field(
    record: &RepositoryRecord,
    field: &str,
) -> Result<i64, BackendError> {
    let raw = required_repository_field(record, field)?;
    raw.parse::<i64>().map_err(|_| {
        BackendError::new(
            BackendErrorKind::Conflict,
            "repository.record.invalid_i64",
            format!(
                "Repository record `{}` field `{field}` was not a valid integer.",
                record.id
            ),
        )
    })
}

fn required_repository_usize_field(
    record: &RepositoryRecord,
    field: &str,
) -> Result<usize, BackendError> {
    let raw = required_repository_field(record, field)?;
    raw.parse::<usize>().map_err(|_| {
        BackendError::new(
            BackendErrorKind::Conflict,
            "repository.record.invalid_usize",
            format!(
                "Repository record `{}` field `{field}` was not a valid positive count.",
                record.id
            ),
        )
    })
}

fn required_repository_bool_field(
    record: &RepositoryRecord,
    field: &str,
) -> Result<bool, BackendError> {
    let raw = required_repository_field(record, field)?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(BackendError::new(
            BackendErrorKind::Conflict,
            "repository.record.invalid_bool",
            format!(
                "Repository record `{}` field `{field}` was not a valid boolean.",
                record.id
            ),
        )),
    }
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
