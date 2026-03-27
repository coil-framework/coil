use super::*;
use axum::response::Response;
use davenda_config::Environment;
use davenda_customer_sdk::{
    AssetWriteRequest, AssetsFacade, AuditFacade, AuthFacade, BackendError, CheckoutHooks,
    CmsHooks, CmsNavigationAppend, CmsPageDraft, CmsPageUpdate, CmsPublishDecision,
    CmsRedirectAppend, CommerceCatalogCollectionUpdate, CommerceCatalogProductUpdate,
    CommerceFacade, CustomerPluginDescriptor, Headers, JobsFacade, OrderDraft, OrderReviewDecision,
    OutboundHttpFacade, RepositoryFacade, RepositoryFacadeExt, RequestContext, VerifiedWebhook,
    VerifiedWebhookAssetHooks, VerifiedWebhookHooks, WebhookHandlingResult,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const LIVE_DATABASE_URL: &str = "postgres://platform:secret@db.internal/platform";
const LIVE_OBJECT_STORE_SECRET: &str = r#"
endpoint_url = "https://s3.internal"
bucket = "runtime"
region = "eu-west-2"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
signed_url_ttl_secs = 900
"#;
const PAYMENT_WEBHOOK_SECRET: &str = "harbor-shop-webhook-secret";
const STRIPE_SECRET_KEY: &str = "sk_test_runtime_placeholder";

type HmacSha256 = Hmac<Sha256>;

struct ObjectStoreTestServer {
    endpoint: String,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ObjectStoreTestServer {
    fn spawn() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let store = Arc::new(Mutex::new(BTreeMap::<String, Vec<u8>>::new()));
        let stop_thread = Arc::clone(&stop);
        let store_thread = Arc::clone(&store);
        let handle = thread::spawn(move || {
            loop {
                if stop_thread.load(Ordering::SeqCst) {
                    break;
                }
                match listener.accept() {
                    Ok((stream, _)) => {
                        let store = Arc::clone(&store_thread);
                        handle_object_store_request(stream, &store);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) => panic!("object-store test server failed: {error}"),
                }
            }
        });
        thread::sleep(Duration::from_millis(25));

        Self {
            endpoint,
            stop,
            handle: Some(handle),
        }
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl Drop for ObjectStoreTestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_object_store_request(
    mut stream: std::net::TcpStream,
    store: &Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
) {
    stream.set_nonblocking(false).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/")
        .trim_start_matches('/')
        .trim_start_matches("runtime/")
        .to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).unwrap();
        let trimmed = header.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some((name, value)) = trimmed.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body).unwrap();
    }

    let (status, headers, response_body) = match method {
        "PUT" => {
            store.lock().unwrap().insert(path, body);
            (
                "200 OK",
                vec![("ETag", "\"davenda-test-etag\"".to_string())],
                Vec::new(),
            )
        }
        "GET" => match store.lock().unwrap().get(&path) {
            Some(bytes) => (
                "200 OK",
                vec![("Content-Type", "application/octet-stream".to_string())],
                bytes.clone(),
            ),
            None => ("404 Not Found", Vec::new(), Vec::new()),
        },
        _ => ("405 Method Not Allowed", Vec::new(), Vec::new()),
    };

    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response_body.len(),
    )
    .unwrap();
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n").unwrap();
    }
    write!(stream, "\r\n").unwrap();
    if !response_body.is_empty() {
        stream.write_all(&response_body).unwrap();
    }
    stream.flush().unwrap();
}

fn object_store_secret(endpoint: &str) -> String {
    format!(
        "endpoint_url = \"{endpoint}\"\n\
bucket = \"runtime\"\n\
region = \"eu-west-2\"\n\
access_key_id = \"runtime-access\"\n\
secret_access_key = \"runtime-secret\"\n\
signed_url_ttl_secs = 900\n"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostedCheckoutCall {
    api_key: String,
    request_body: String,
    idempotency_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostedCheckoutSessionRecord {
    id: String,
    status: Option<String>,
    payment_status: Option<String>,
    payment_reference: Option<String>,
}

#[derive(Debug)]
struct StaticHostedCheckoutClient {
    session_id: String,
    session_url: String,
    default_status: Option<String>,
    default_payment_status: Option<String>,
    calls: Mutex<Vec<HostedCheckoutCall>>,
    sessions: Mutex<BTreeMap<String, HostedCheckoutSessionRecord>>,
}

impl StaticHostedCheckoutClient {
    fn with_url(session_url: &str) -> Self {
        Self::with_url_and_status(session_url, Some("open"), Some("unpaid"))
    }

    fn with_url_and_status(
        session_url: &str,
        status: Option<&str>,
        payment_status: Option<&str>,
    ) -> Self {
        let session_id = session_url
            .rsplit('/')
            .next()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("cs_test_harbor_shop")
            .to_string();
        Self {
            session_id,
            session_url: session_url.to_string(),
            default_status: status.map(str::to_string),
            default_payment_status: payment_status.map(str::to_string),
            calls: Mutex::new(Vec::new()),
            sessions: Mutex::new(BTreeMap::new()),
        }
    }

    fn take_calls(&self) -> Vec<HostedCheckoutCall> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }
}

impl crate::server::HostedCheckoutClient for StaticHostedCheckoutClient {
    fn create_stripe_checkout_session(
        &self,
        api_key: &str,
        request_body: &str,
        idempotency_key: &str,
    ) -> Result<crate::server::HostedCheckoutSession, String> {
        let payment_reference = url::form_urlencoded::parse(request_body.as_bytes())
            .find_map(|(key, value)| (key == "client_reference_id").then(|| value.into_owned()));
        self.calls.lock().unwrap().push(HostedCheckoutCall {
            api_key: api_key.to_string(),
            request_body: request_body.to_string(),
            idempotency_key: idempotency_key.to_string(),
        });
        self.sessions.lock().unwrap().insert(
            self.session_id.clone(),
            HostedCheckoutSessionRecord {
                id: self.session_id.clone(),
                status: self.default_status.clone(),
                payment_status: self.default_payment_status.clone(),
                payment_reference,
            },
        );
        Ok(crate::server::HostedCheckoutSession {
            id: self.session_id.clone(),
            url: self.session_url.clone(),
        })
    }

    fn fetch_stripe_checkout_session(
        &self,
        _api_key: &str,
        session_id: &str,
    ) -> Result<crate::server::HostedCheckoutSessionStatus, String> {
        let session = self
            .sessions
            .lock()
            .unwrap()
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("unknown checkout session `{session_id}`"))?;
        Ok(crate::server::HostedCheckoutSessionStatus {
            id: session.id,
            status: session.status,
            payment_status: session.payment_status,
            payment_reference: session.payment_reference,
        })
    }
}

#[derive(Debug)]
struct RejectMembershipCheckoutPlugin;

#[derive(Debug)]
struct RejectMembershipCheckoutHooks;

impl CheckoutHooks for RejectMembershipCheckoutHooks {
    fn review_order(
        &self,
        _ctx: &RequestContext,
        order: &OrderDraft,
        _commerce: &dyn CommerceFacade,
        _auth: &dyn AuthFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<OrderReviewDecision, BackendError> {
        if order
            .lines
            .iter()
            .any(|line| line.entitlement_key.as_deref() == Some("membership.gold"))
        {
            return Ok(OrderReviewDecision::rejected(
                "checkout.manual_review",
                "Orders containing Gold Membership require manual review before payment can start.",
            ));
        }
        Ok(OrderReviewDecision::approved())
    }
}

impl CustomerBackendPlugin for RejectMembershipCheckoutPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-checkout-policy",
            "Harbor Shop Checkout Policy",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_checkout_hooks(Arc::new(RejectMembershipCheckoutHooks))
    }
}

#[derive(Debug)]
struct RejectCmsPublishPlugin;

#[derive(Debug)]
struct RejectCmsPublishHooks;

#[derive(Debug)]
struct RewriteCmsPublishPlugin;

#[derive(Debug)]
struct RewriteCmsPublishHooks;

#[derive(Debug)]
struct RewriteCmsWorkspacePublishPlugin;

#[derive(Debug)]
struct RewriteCmsWorkspacePublishHooks;

impl CmsHooks for RejectCmsPublishHooks {
    fn validate_page_publish(
        &self,
        _ctx: &RequestContext,
        draft: &CmsPageDraft,
        repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<CmsPublishDecision, BackendError> {
        let seen_slug = repositories
            .cms_page(&draft.page_id)?
            .map(|record| record.slug)
            .unwrap_or_default();
        if seen_slug != draft.slug {
            return Err(BackendError::new(
                davenda_customer_sdk::BackendErrorKind::Conflict,
                "cms.page.repository_mismatch",
                "linked CMS hook did not see the draft page that is about to publish",
            ));
        }
        Ok(CmsPublishDecision::reject(
            "cms.page.requires-review",
            "Linked customer policy blocked this page from publishing until editorial review is complete.",
        ))
    }
}

impl CmsHooks for RewriteCmsPublishHooks {
    fn validate_page_publish(
        &self,
        _ctx: &RequestContext,
        draft: &CmsPageDraft,
        repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<CmsPublishDecision, BackendError> {
        repositories.update_cms_page(&CmsPageUpdate::new(
            draft.page_id.clone(),
            format!("{} (Linked review)", draft.title),
            draft.slug.clone(),
            "Linked customer review updated this page before publish.",
            draft.body_html.clone(),
        ))?;
        Ok(CmsPublishDecision::Allow)
    }
}

impl CmsHooks for RewriteCmsWorkspacePublishHooks {
    fn validate_page_publish(
        &self,
        _ctx: &RequestContext,
        draft: &CmsPageDraft,
        repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<CmsPublishDecision, BackendError> {
        let navigation = repositories.cms_navigation_items()?;
        if navigation.is_empty() {
            return Err(BackendError::new(
                davenda_customer_sdk::BackendErrorKind::Conflict,
                "cms.navigation.missing",
                "linked CMS hook expected the live primary navigation to exist",
            ));
        }
        let redirects = repositories.cms_redirects()?;
        if redirects
            .iter()
            .any(|record| record.from == "/legacy/shipping")
        {
            return Err(BackendError::new(
                davenda_customer_sdk::BackendErrorKind::Conflict,
                "cms.redirects.duplicate",
                "linked CMS hook found an unexpected legacy shipping redirect before publish",
            ));
        }
        repositories.append_cms_navigation_item(&CmsNavigationAppend::new(
            "Shipping",
            format!("/pages/{}", draft.slug),
        ))?;
        repositories.append_cms_redirect(&CmsRedirectAppend::new(
            "/legacy/shipping",
            format!("/pages/{}", draft.slug),
            true,
        ))?;
        Ok(CmsPublishDecision::Allow)
    }
}

impl CustomerBackendPlugin for RejectCmsPublishPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new("harbor-shop-cms-policy", "Harbor Shop CMS Policy", "0.1.0")
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_cms_hooks(Arc::new(RejectCmsPublishHooks))
    }
}

impl CustomerBackendPlugin for RewriteCmsPublishPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-cms-publish-rewriter",
            "Harbor Shop CMS Publish Rewriter",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_cms_hooks(Arc::new(RewriteCmsPublishHooks))
    }
}

impl CustomerBackendPlugin for RewriteCmsWorkspacePublishPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-cms-workspace-rewriter",
            "Harbor Shop CMS Workspace Rewriter",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_cms_hooks(Arc::new(RewriteCmsWorkspacePublishHooks))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedVerifiedWebhook {
    source: String,
    event: String,
    headers: Headers,
    content_type: Option<String>,
    payload: String,
}

#[derive(Debug)]
struct RecordingVerifiedWebhookPlugin {
    calls: Arc<Mutex<Vec<RecordedVerifiedWebhook>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedVerifiedWebhookAssetWrite {
    logical_path: String,
    storage_class: String,
    storage_path: String,
    bytes_written: u64,
    public_url: Option<String>,
}

#[derive(Debug)]
struct RecordingVerifiedWebhookAssetPlugin {
    writes: Arc<Mutex<Vec<RecordedVerifiedWebhookAssetWrite>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedInspectedManagedAsset {
    logical_path: String,
    storage_class: String,
    public_url: Option<String>,
}

#[derive(Debug)]
struct InspectingVerifiedWebhookAssetPlugin {
    inspected: Arc<Mutex<Vec<RecordedInspectedManagedAsset>>>,
}

#[derive(Debug)]
struct WritingThenInspectingVerifiedWebhookAssetPlugin {
    writes: Arc<Mutex<Vec<RecordedVerifiedWebhookAssetWrite>>>,
    inspected: Arc<Mutex<Vec<RecordedInspectedManagedAsset>>>,
}

impl VerifiedWebhookHooks for RecordingVerifiedWebhookPlugin {
    fn handle_verified_webhook(
        &self,
        _ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        jobs: &dyn JobsFacade,
        _repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        let receipt = jobs.enqueue(
            davenda_customer_sdk::JobRequest::new(
                "jobs.work",
                "ops.report.export",
                "record verified payment webhook",
            )
            .with_idempotency_key(format!(
                "verified-webhook:{}:{}",
                webhook.source, webhook.event
            )),
        )?;
        self.calls.lock().unwrap().push(RecordedVerifiedWebhook {
            source: webhook.source.clone(),
            event: webhook.event.clone(),
            headers: webhook.headers.clone(),
            content_type: webhook.content_type.clone(),
            payload: String::from_utf8_lossy(&webhook.payload).into_owned(),
        });
        Ok(WebhookHandlingResult::accepted(Some(format!(
            "{}:{}",
            receipt.queue, receipt.job_id
        ))))
    }
}

impl VerifiedWebhookAssetHooks for RecordingVerifiedWebhookAssetPlugin {
    fn handle_verified_webhook(
        &self,
        ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        _repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
        assets: &dyn AssetsFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        let logical_path = format!(
            "uploads/customer-hooks/{}/{}.json",
            ctx.customer_app.app_id, webhook.event
        );
        let receipt = assets.publish(AssetWriteRequest {
            logical_path: logical_path.clone(),
            storage_class: "public_upload".to_string(),
            content_type: Some("application/json".to_string()),
            bytes: webhook.payload.clone(),
            metadata: BTreeMap::new(),
        })?;
        let asset = assets.inspect(&logical_path)?.ok_or_else(|| {
            BackendError::new(
                davenda_customer_sdk::BackendErrorKind::Internal,
                "asset.inspect.missing",
                "linked customer asset hook could not inspect the asset it just wrote",
            )
        })?;
        self.writes
            .lock()
            .unwrap()
            .push(RecordedVerifiedWebhookAssetWrite {
                logical_path: asset.logical_path.clone(),
                storage_class: asset.storage_class.clone(),
                storage_path: receipt.storage_path,
                bytes_written: receipt.bytes_written,
                public_url: asset.public_url.clone(),
            });
        Ok(WebhookHandlingResult::accepted(Some(format!(
            "asset:{}",
            asset.logical_path
        ))))
    }
}

impl VerifiedWebhookAssetHooks for InspectingVerifiedWebhookAssetPlugin {
    fn handle_verified_webhook(
        &self,
        ctx: &RequestContext,
        _webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        _repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
        assets: &dyn AssetsFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        let logical_path = format!(
            "uploads/customer-hooks/{}/payment.captured.json",
            ctx.customer_app.app_id
        );
        let asset = assets.inspect(&logical_path)?.ok_or_else(|| {
            BackendError::new(
                davenda_customer_sdk::BackendErrorKind::Internal,
                "asset.inspect.missing",
                "linked customer asset hook could not inspect the persisted asset",
            )
        })?;
        self.inspected
            .lock()
            .unwrap()
            .push(RecordedInspectedManagedAsset {
                logical_path: asset.logical_path.clone(),
                storage_class: asset.storage_class.clone(),
                public_url: asset.public_url.clone(),
            });
        Ok(WebhookHandlingResult::accepted(Some(format!(
            "inspected:{}",
            asset.logical_path
        ))))
    }
}

impl VerifiedWebhookAssetHooks for WritingThenInspectingVerifiedWebhookAssetPlugin {
    fn handle_verified_webhook(
        &self,
        ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        _repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
        assets: &dyn AssetsFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        let written_path = format!(
            "uploads/customer-hooks/{}/payment.captured.json",
            ctx.customer_app.app_id
        );
        if webhook.event == "payment.captured" {
            let receipt = assets.publish(AssetWriteRequest {
                logical_path: written_path.clone(),
                storage_class: "public_upload".to_string(),
                content_type: Some("application/json".to_string()),
                bytes: webhook.payload.clone(),
                metadata: BTreeMap::new(),
            })?;
            let asset = assets.inspect(&written_path)?.ok_or_else(|| {
                BackendError::new(
                    davenda_customer_sdk::BackendErrorKind::Internal,
                    "asset.inspect.missing",
                    "linked customer asset hook could not inspect the asset it just wrote",
                )
            })?;
            self.writes
                .lock()
                .unwrap()
                .push(RecordedVerifiedWebhookAssetWrite {
                    logical_path: asset.logical_path.clone(),
                    storage_class: asset.storage_class.clone(),
                    storage_path: receipt.storage_path,
                    bytes_written: receipt.bytes_written,
                    public_url: asset.public_url.clone(),
                });
            return Ok(WebhookHandlingResult::accepted(Some(format!(
                "asset:{}",
                asset.logical_path
            ))));
        }

        let asset = assets.inspect(&written_path)?.ok_or_else(|| {
            BackendError::new(
                davenda_customer_sdk::BackendErrorKind::Internal,
                "asset.inspect.missing",
                "linked customer asset hook could not inspect the persisted asset",
            )
        })?;
        self.inspected
            .lock()
            .unwrap()
            .push(RecordedInspectedManagedAsset {
                logical_path: asset.logical_path.clone(),
                storage_class: asset.storage_class.clone(),
                public_url: asset.public_url.clone(),
            });
        Ok(WebhookHandlingResult::accepted(Some(format!(
            "inspected:{}",
            asset.logical_path
        ))))
    }
}

#[derive(Debug)]
struct RejectVerifiedWebhookPlugin;

impl VerifiedWebhookHooks for RejectVerifiedWebhookPlugin {
    fn handle_verified_webhook(
        &self,
        _ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        _repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        Ok(WebhookHandlingResult::rejected(
            "customer.policy.rejected",
            format!(
                "linked customer policy rejected {}:{}",
                webhook.source, webhook.event
            ),
        ))
    }
}

impl CustomerBackendPlugin for RejectVerifiedWebhookPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-verified-webhook-rejector",
            "Harbor Shop Verified Webhook Rejector",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_hooks(Arc::new(Self))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedWebhookOrderRead {
    order_id: String,
    status: String,
    payment_reference: String,
    checkout_email: String,
}

#[derive(Debug)]
struct RecordingWebhookOrderRepositoryPlugin {
    reads: Arc<Mutex<Vec<RecordedWebhookOrderRead>>>,
}

impl VerifiedWebhookHooks for RecordingWebhookOrderRepositoryPlugin {
    fn handle_verified_webhook(
        &self,
        _ctx: &RequestContext,
        webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        let order = repositories
            .commerce_order_by_payment_reference("PAY-50001")?
            .ok_or_else(|| {
                BackendError::new(
                    davenda_customer_sdk::BackendErrorKind::Conflict,
                    "repository.order.missing",
                    "linked verified webhook hook did not see the persisted order",
                )
            })?;
        self.reads.lock().unwrap().push(RecordedWebhookOrderRead {
            order_id: order.order_id.clone(),
            status: order.status.clone(),
            payment_reference: order.payment_reference.clone().unwrap_or_default(),
            checkout_email: order.checkout_email.clone().unwrap_or_default(),
        });
        Ok(WebhookHandlingResult::accepted(Some(order.order_id)))
    }
}

impl CustomerBackendPlugin for RecordingWebhookOrderRepositoryPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-webhook-order-reader",
            "Harbor Shop Webhook Order Reader",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_hooks(Arc::new(Self {
            reads: Arc::clone(&self.reads),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedWebhookCatalogMutation {
    product_handle: String,
    product_title: String,
    collection_handle: String,
    collection_label: String,
}

#[derive(Debug)]
struct RecordingWebhookCatalogRepositoryPlugin {
    mutations: Arc<Mutex<Vec<RecordedWebhookCatalogMutation>>>,
}

impl VerifiedWebhookHooks for RecordingWebhookCatalogRepositoryPlugin {
    fn handle_verified_webhook(
        &self,
        _ctx: &RequestContext,
        _webhook: &VerifiedWebhook,
        _http: &dyn OutboundHttpFacade,
        _jobs: &dyn JobsFacade,
        repositories: &dyn RepositoryFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<WebhookHandlingResult, BackendError> {
        let product = repositories
            .commerce_catalog_product("gold-membership")?
            .ok_or_else(|| {
                BackendError::new(
                    davenda_customer_sdk::BackendErrorKind::Conflict,
                    "repository.catalog.product_missing",
                    "linked verified webhook hook did not see the effective catalog product",
                )
            })?;
        let collection = repositories
            .commerce_catalog_collection(&product.collection_handle)?
            .ok_or_else(|| {
                BackendError::new(
                    davenda_customer_sdk::BackendErrorKind::Conflict,
                    "repository.catalog.collection_missing",
                    "linked verified webhook hook did not see the effective catalog collection",
                )
            })?;

        let updated_product = CommerceCatalogProductUpdate::new(
            product.handle.clone(),
            "Gold Membership Plus",
            "Webhook-adjusted premium access for Harbor members.",
            product.price_minor + 500,
            product.collection_handle.clone(),
            product.is_visible,
        );
        repositories.update_commerce_catalog_product(&updated_product)?;

        let updated_collection = CommerceCatalogCollectionUpdate::new(
            collection.handle.clone(),
            collection.title.clone(),
            "Premium perks",
            "Webhook-adjusted membership merchandising copy.",
            collection.is_visible,
        );
        repositories.update_commerce_catalog_collection(&updated_collection)?;

        self.mutations
            .lock()
            .unwrap()
            .push(RecordedWebhookCatalogMutation {
                product_handle: product.handle,
                product_title: updated_product.title,
                collection_handle: collection.handle,
                collection_label: updated_collection.label,
            });
        Ok(WebhookHandlingResult::accepted(Some(
            "catalog-overrides-updated".to_string(),
        )))
    }
}

impl CustomerBackendPlugin for RecordingWebhookCatalogRepositoryPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-webhook-catalog-repository",
            "Harbor Shop Webhook Catalog Repository",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_hooks(Arc::new(Self {
            mutations: Arc::clone(&self.mutations),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedCheckoutLineMetadata {
    sku: String,
    metadata: BTreeMap<String, String>,
}

#[derive(Debug)]
struct RecordingCheckoutLineMetadataPlugin {
    calls: Arc<Mutex<Vec<RecordedCheckoutLineMetadata>>>,
}

impl CheckoutHooks for RecordingCheckoutLineMetadataPlugin {
    fn review_order(
        &self,
        _ctx: &RequestContext,
        order: &OrderDraft,
        _commerce: &dyn CommerceFacade,
        _auth: &dyn AuthFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<OrderReviewDecision, BackendError> {
        let mut calls = self.calls.lock().unwrap();
        calls.extend(order.lines.iter().map(|line| RecordedCheckoutLineMetadata {
            sku: line.sku.clone(),
            metadata: line.metadata.clone(),
        }));
        Ok(OrderReviewDecision::approved())
    }
}

impl CustomerBackendPlugin for RecordingCheckoutLineMetadataPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-checkout-line-metadata-recorder",
            "Harbor Shop Checkout Line Metadata Recorder",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_checkout_hooks(Arc::new(Self {
            calls: Arc::clone(&self.calls),
        }))
    }
}

impl CustomerBackendPlugin for RecordingVerifiedWebhookPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-verified-webhooks",
            "Harbor Shop Verified Webhooks",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_hooks(Arc::new(Self {
            calls: self.calls.clone(),
        }))
    }
}

impl CustomerBackendPlugin for RecordingVerifiedWebhookAssetPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-verified-webhook-assets",
            "Harbor Shop Verified Webhook Assets",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_asset_hooks(Arc::new(Self {
            writes: self.writes.clone(),
        }))
    }
}

impl CustomerBackendPlugin for InspectingVerifiedWebhookAssetPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-verified-webhook-asset-inspector",
            "Harbor Shop Verified Webhook Asset Inspector",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_asset_hooks(Arc::new(Self {
            inspected: self.inspected.clone(),
        }))
    }
}

impl CustomerBackendPlugin for WritingThenInspectingVerifiedWebhookAssetPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new(
            "harbor-shop-verified-webhook-asset-write-inspect",
            "Harbor Shop Verified Webhook Asset Write And Inspect",
            "0.1.0",
        )
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_verified_webhook_asset_hooks(Arc::new(Self {
            writes: self.writes.clone(),
            inspected: self.inspected.clone(),
        }))
    }
}

fn live_backend_secret_resolver_with_object_store_secret(
    object_store_secret: &str,
) -> StaticSecretResolver {
    StaticSecretResolver::new()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "DATABASE_URL".to_string(),
            },
            LIVE_DATABASE_URL,
        )
        .unwrap()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "OBJECT_STORE_URL".to_string(),
            },
            object_store_secret,
        )
        .unwrap()
}

fn live_backend_secret_resolver() -> StaticSecretResolver {
    live_backend_secret_resolver_with_object_store_secret(LIVE_OBJECT_STORE_SECRET)
}

fn live_backend_secret_resolver_with_payment_webhook_and_object_store_secret(
    object_store_secret: &str,
) -> StaticSecretResolver {
    live_backend_secret_resolver_with_object_store_secret(object_store_secret)
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "PAYMENT_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "STRIPE_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "STRIPE_SECRET_KEY".to_string(),
            },
            STRIPE_SECRET_KEY,
        )
        .unwrap()
}

fn live_backend_secret_resolver_with_payment_webhook() -> StaticSecretResolver {
    live_backend_secret_resolver_with_payment_webhook_and_object_store_secret(
        LIVE_OBJECT_STORE_SECRET,
    )
}

fn live_backend_secret_resolver_with_placeholder_stripe() -> StaticSecretResolver {
    live_backend_secret_resolver()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "PAYMENT_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "STRIPE_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "STRIPE_SECRET_KEY".to_string(),
            },
            "sk_test_replace_me",
        )
        .unwrap()
}

fn with_payment_webhook_secret(config: PlatformConfig) -> PlatformConfig {
    PlatformConfig::from_toml_str(&format!(
        "{}\n[modules.commerce]\npayment_webhook_secret = {{ kind = \"env\", var = \"PAYMENT_WEBHOOK_SECRET\" }}\n",
        VALID_CONFIG.replace(
            "name = \"showcase-events\"",
            &format!("name = \"{}\"", config.app.name),
        )
    ))
    .unwrap()
}

fn with_stripe_payment_provider(config: PlatformConfig) -> PlatformConfig {
    let mut config = config;
    config.modules.enabled = vec![
        "commerce".to_string(),
        "commerce-payments-stripe".to_string(),
    ];
    let mut stripe_settings = toml::Table::new();
    stripe_settings.insert(
        "provider".to_string(),
        toml::Value::String("stripe".to_string()),
    );
    stripe_settings.insert(
        "checkout_mode".to_string(),
        toml::Value::String("webhook-confirmation".to_string()),
    );
    stripe_settings.insert(
        "webhook_secret".to_string(),
        toml::Value::try_from(davenda_config::SecretRef::Env {
            var: "STRIPE_WEBHOOK_SECRET".to_string(),
        })
        .unwrap(),
    );
    config.modules.settings.insert(
        "commerce-payments-stripe".to_string(),
        toml::Value::Table(stripe_settings),
    );
    config
}

fn checked_in_harbor_shop_config(app_name: &str) -> PlatformConfig {
    let mut config =
        PlatformConfig::from_file(checked_in_harbor_shop_root().join("platform.toml")).unwrap();
    config.app.name = app_name.to_string();
    config.storage.local_root = std::env::temp_dir()
        .join(format!("davenda-runtime-{app_name}"))
        .display()
        .to_string();
    config
}

fn payment_webhook_signature(provider: &str, event: &str, payment_reference: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(PAYMENT_WEBHOOK_SECRET.as_bytes()).expect("valid hmac key");
    mac.update(provider.as_bytes());
    mac.update(b":");
    mac.update(event.as_bytes());
    mac.update(b":");
    mac.update(payment_reference.as_bytes());
    format!("{:x}", mac.finalize().into_bytes())
}

fn stripe_webhook_signature(timestamp: i64, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(PAYMENT_WEBHOOK_SECRET.as_bytes()).expect("valid hmac key");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload.as_bytes());
    format!("t={timestamp},v1={:x}", mac.finalize().into_bytes())
}

fn fresh_stripe_webhook_signature(payload: &str) -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    stripe_webhook_signature(timestamp, payload)
}

fn response_header(response: &Response<Body>, name: &str) -> String {
    response
        .headers()
        .get(name)
        .unwrap_or_else(|| panic!("missing response header `{name}`"))
        .to_str()
        .unwrap()
        .to_string()
}

fn response_session_cookie(response: &Response<Body>) -> String {
    let header = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find(|value| value.starts_with("davenda_session="))
        .expect("response should include a davenda_session cookie");
    cookie_value(header)
}

fn cookie_pair_from_response(response: &Response<Body>, name: &str) -> Option<String> {
    cookie_pair_from_headers(response.headers(), name)
}

fn cookie_pair_from_headers(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(axum::http::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|header: &str| {
            let prefix = format!("{name}=");
            header
                .strip_prefix(&prefix)
                .and_then(|rest: &str| rest.split(';').next())
                .map(|value| format!("{name}={value}"))
        })
}

fn storefront_csrf_token_from_body(body: &str, action: &str) -> String {
    let needle = format!("\"{action}\":\"");
    let start = body
        .find(&needle)
        .unwrap_or_else(|| panic!("missing storefront csrf token for `{action}`"))
        + needle.len();
    let rest = &body[start..];
    let end = rest
        .find('"')
        .unwrap_or_else(|| panic!("missing closing quote for storefront csrf token `{action}`"));
    rest[..end].to_string()
}

fn cms_form_csrf_token_from_body(body: &str, action: &str) -> String {
    let action_attr = format!("action=\"{action}\"");
    let action_index = body
        .find(&action_attr)
        .unwrap_or_else(|| panic!("missing CMS admin form action `{action}`"));
    let form_start = body[..action_index]
        .rfind("<form")
        .unwrap_or_else(|| panic!("missing form start for CMS admin action `{action}`"));
    let form_end = body[action_index..]
        .find("</form>")
        .map(|relative| action_index + relative)
        .unwrap_or_else(|| panic!("missing form end for CMS admin action `{action}`"));
    let form_html = &body[form_start..form_end];
    let token_attr = "name=\"_csrf\" value=\"";
    let token_start = form_html
        .find(token_attr)
        .unwrap_or_else(|| panic!("missing CMS admin csrf token for `{action}`"))
        + token_attr.len();
    let token_rest = &form_html[token_start..];
    let token_end = token_rest
        .find('"')
        .unwrap_or_else(|| panic!("missing closing quote for CMS admin csrf token `{action}`"));
    token_rest[..token_end].to_string()
}

fn unique_app_name(label: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{label}-{unique}")
}

fn checked_in_harbor_shop_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/harbor-shop")
}

#[tokio::test]
async fn server_router_keeps_public_probes_open_and_diagnostics_privileged() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "DAVENDA_PAYMENT_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let health = server
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let health_body = String::from_utf8(
        to_bytes(health.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(health_body.contains("\"liveness\""), true);
    assert_eq!(health_body.contains("\"readiness\""), true);

    let readiness = server
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(readiness.status(), StatusCode::OK);

    let readiness_alias = server
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/readiness")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(readiness_alias.status(), StatusCode::OK);

    let metrics = server
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let metrics_body = String::from_utf8(
        to_bytes(metrics.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(metrics_body.contains("davenda.http.request.latency_ms"));
    assert!(metrics_body.contains("\"metrics_enabled\":true"));

    let public_diagnostics = server
        .public_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/diagnostics")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(public_diagnostics.status(), StatusCode::NOT_FOUND);

    let diagnostics = server
        .router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/diagnostics")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(diagnostics.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn server_router_denies_diagnostics_probe_for_authenticated_sessions_without_audit_access() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "DAVENDA_PAYMENT_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(StaticLiveRouteCapabilityAuthorizer::new()),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let _ = server.wasm_host().prepare_webhook_invocation(
        "commerce.payment-provider",
        "payment.authorized",
        false,
        true,
        "trace.webhooks.verification-failed",
        ExtensionPrincipal::service_account("commerce.webhooks"),
    );
    let diagnostics = server
        .privileged_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/diagnostics")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(diagnostics.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn server_router_allows_diagnostics_probe_for_admin_audit_read_access() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "DAVENDA_PAYMENT_WEBHOOK_SECRET".to_string(),
            },
            PAYMENT_WEBHOOK_SECRET,
        )
        .unwrap();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let authorizer = Arc::new(StaticLiveRouteCapabilityAuthorizer::new().allowing(
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user("operator-live-1")),
        Capability::AdminAuditRead,
        davenda_auth::Entity::admin_module("showcase-events"),
    ));
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        authorizer,
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let diagnostics = server
        .privileged_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/diagnostics")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = diagnostics.status();
    let diagnostics_body = String::from_utf8(
        to_bytes(diagnostics.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert!(diagnostics_body.contains("\"customer_app\""));
    assert!(diagnostics_body.contains("\"database\""));
    assert!(diagnostics_body.contains("\"metadata\""));
    assert!(diagnostics_body.contains("\"extensions\""));
    assert!(diagnostics_body.contains("\"webhooks\""));
    assert!(diagnostics_body.contains("\"verification_failed\""));
    assert!(diagnostics_body.contains("\"backend\":\"local-sqlite\""));
    assert!(diagnostics_body.contains("\"path\""));
}

#[tokio::test]
async fn server_router_bootstraps_development_admin_session_from_dev_route() {
    let app_name = unique_app_name("harbor-shop-runtime-dev-login");
    let template_root = checked_in_harbor_shop_root();
    let mut config = checked_in_harbor_shop_config(&app_name);
    config.app.environment = davenda_config::Environment::Development;
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(AdminModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(StaticLiveRouteCapabilityAuthorizer::new()),
    )
    .unwrap();
    let mut login_request = Request::builder()
        .method("GET")
        .uri("/__dev/login/admin?next=/admin")
        .header("host", "localhost:8080")
        .body(Body::empty())
        .unwrap();
    login_request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(
            "127.0.0.1:12345".parse::<std::net::SocketAddr>().unwrap(),
        ));
    let login_response = server.router().oneshot(login_request).await.unwrap();
    assert_eq!(login_response.status(), StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(response_header(&login_response, "location"), "/admin");
    let session_cookie =
        cookie_pair_from_response(&login_response, "davenda_session").expect("dev login cookie");

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .header("host", "localhost:8080")
                .header("cookie", session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_status = admin_response.status();
    let admin_body = String::from_utf8(
        to_bytes(admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(admin_status, StatusCode::OK, "{admin_body}");
    assert!(admin_body.contains("Harbor Shop Admin"), "{admin_body}");
    assert!(admin_body.contains("dev-admin"), "{admin_body}");
}

#[tokio::test]
async fn server_router_hides_auth_explain_when_deployment_disables_it() {
    let mut config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    config.auth.explain_api = false;
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(StaticLiveRouteCapabilityAuthorizer::new()),
    )
    .unwrap();

    let response = server
        .privileged_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/diagnostics/auth/explain")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn server_router_serves_live_auth_explain_when_enabled_and_authorized() {
    let mut config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    config.auth.explain_api = true;
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let package = DefaultAuthModelPackage::default();
    let capability = Capability::CmsPageRead;
    let subject =
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user("operator-live-1"));
    let resource = davenda_auth::Entity::page("homepage");
    let explanation = davenda_auth::CapabilityExplanation {
        manifest: package.manifest().clone(),
        subject: subject.clone(),
        capability,
        object: resource.clone(),
        binding: package.binding_for(capability).unwrap().clone(),
        decision: davenda_auth::ExplainDecision::Allow,
        options: davenda_auth::ExplainOptions::default(),
        trace: davenda_auth::ExplainTrace::Allowed(davenda_auth::AllowedExplanation {
            steps: vec![davenda_auth::ExplainStep::Start {
                node: davenda_auth::ExplainedNode {
                    object: resource.clone(),
                    relation: None,
                },
            }],
        }),
    };
    let explainer = StaticLiveAuthExplainer::new(explanation.clone());
    let authorizer = Arc::new(StaticLiveRouteCapabilityAuthorizer::new().allowing(
        subject.clone(),
        Capability::AdminAuditRead,
        davenda_auth::Entity::admin_module("showcase-events"),
    ));
    let server = HttpServerHost::new_with_authorizer_and_explainer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        authorizer,
        Arc::new(explainer.clone()),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let response = server
        .privileged_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/diagnostics/auth/explain")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "subject": "user:alice",
                        "capability": "cms.page.read",
                        "resource": "page:homepage",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(payload["tenant_id"], serde_json::json!(101));
    assert_eq!(payload["subject"], serde_json::json!("user:alice"));
    assert_eq!(payload["decision"], serde_json::json!("allow"));
    let requests = explainer.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].subject,
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user("alice"))
    );
    assert_eq!(requests[0].capability, Capability::CmsPageRead);
    assert_eq!(requests[0].object, davenda_auth::Entity::page("homepage"));
    assert!(requests[0].options.cycle_protection);
}

#[tokio::test]
async fn server_router_uses_live_auth_explainer_when_enabled() {
    let mut config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    config.auth.explain_api = true;
    config.database.url = None;
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let subject =
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user("operator-live-1"));
    let authorizer = Arc::new(StaticLiveRouteCapabilityAuthorizer::new().allowing(
        subject.clone(),
        Capability::AdminAuditRead,
        davenda_auth::Entity::admin_module("showcase-events"),
    ));
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        authorizer,
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();

    let response = server
        .privileged_router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/diagnostics/auth/explain")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "subject": "user:alice",
                        "capability": "cms.page.read",
                        "resource": "page:homepage",
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(body.contains("auth explain failed"));
    assert!(body.contains("live auth backend"));
}

#[test]
fn runtime_plan_selects_local_sqlite_metadata_audit_backend_in_single_node_mode() {
    let plan = RuntimeBuilder::new(
        PlatformConfig::from_toml_str(VALID_CONFIG).unwrap(),
        DefaultAuthModelPackage::default(),
    )
    .build()
    .unwrap();

    match plan.metadata_audit_backend_selection() {
        crate::plan::MetadataAuditBackendSelection::LocalSqlite { root, namespace } => {
            assert_eq!(root, std::path::PathBuf::from("/tmp/davenda-runtime-tests"));
            assert_eq!(namespace, plan.shared_backend_namespace());
        }
        other => panic!("expected local sqlite metadata backend, got {other:?}"),
    }
}

#[test]
fn runtime_plan_uses_shared_postgres_metadata_audit_backend_in_distributed_mode() {
    let mut config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    config.storage.deployment = StorageDeployment::Distributed;
    config.storage.single_node_escape_hatch = davenda_config::SingleNodeStorageMode::Disabled;
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let host = plan.wasm_host();

    match plan.metadata_audit_backend_selection() {
        crate::plan::MetadataAuditBackendSelection::SharedPostgres { runtime } => {
            assert_eq!(runtime.schema, plan.data.schema);
        }
        other => panic!("expected shared postgres metadata backend, got {other:?}"),
    }
    assert_eq!(host.metadata_audit_backend_kind(), "shared-postgres");
    assert_eq!(
        host.metadata_audit_location(),
        "shared-postgres:public.metadata_audit_entries"
    );
}

#[tokio::test]
async fn server_host_rejects_request_bodies_over_the_configured_limit_before_handling() {
    let mut config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    config.server.max_body_bytes = Some(8);
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Post, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(
            HandlerDefinition::json("account.dashboard", std::collections::BTreeMap::new())
                .unwrap(),
        )
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/account")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("quantity=1000"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn server_host_accepts_form_body_csrf_tokens_for_state_changing_browser_routes() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("cart.update", HttpMethod::Post, "/cart")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(
            HandlerDefinition::json(
                "cart.update",
                BTreeMap::from([("status".to_string(), "updated".to_string())]),
            )
            .unwrap(),
        )
        .build()
        .unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-form")
                .unwrap(),
            now,
        )
        .unwrap();
    let token = plan
        .browser
        .csrf
        .issue_token(csrf_secret, &issued.record.session_id, "cart.update")
        .unwrap();
    let body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &token)
        .append_pair("line_id", "sku-1")
        .append_pair("quantity", "2")
        .finish();

    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart?coupon=SPRING24")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .header(
                    "content-type",
                    "application/x-www-form-urlencoded; charset=utf-8",
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-davenda-route").unwrap(),
        "cart.update"
    );
}

#[tokio::test]
async fn server_host_does_not_bypass_session_auth_for_form_posts() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("cart.update", HttpMethod::Post, "/cart")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(
            HandlerDefinition::json(
                "cart.update",
                BTreeMap::from([("status".to_string(), "updated".to_string())]),
            )
            .unwrap(),
        )
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("_csrf=forged&line_id=sku-1&quantity=2"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn server_host_adapts_live_requests_into_runtime_execution() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .with_template(page_template(customer_namespace, "account/dashboard"))
        .build()
        .unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/account")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-davenda-route").unwrap(),
        "account.dashboard"
    );
    assert_eq!(response.headers().get("x-davenda-locale").unwrap(), "en-GB");
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        "private, max-age=60, stale-while-revalidate=30"
    );
}

#[tokio::test]
async fn server_host_uses_live_browser_host_wiring_for_shared_sessions() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .with_template(page_template(customer_namespace, "account/dashboard"))
        .build()
        .unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let sibling = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = sibling
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-2")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/account")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-davenda-route").unwrap(),
        "account.dashboard"
    );
}

#[tokio::test]
async fn server_host_authorizes_capability_routes_through_live_authorizer() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_template(fragment_template(customer_namespace, "cms/preview"))
        .build()
        .unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let authorizer = Arc::new(StaticLiveRouteCapabilityAuthorizer::new().allowing(
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user("editor-live-1")),
        Capability::CmsPageRead,
        davenda_auth::Entity::page("http.surface.module.cms.page.cms.preview"),
    ));
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        cookie_secret.to_vec(),
        csrf_secret.to_vec(),
        authorizer.clone(),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("editor-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/admin/pages/preview")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("x-davenda-route").unwrap(),
        "cms.preview"
    );
    assert_eq!(
        authorizer.checks(),
        vec![LiveAuthorizationCheck {
            subject: davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(
                "editor-live-1",
            )),
            capability: Capability::CmsPageRead,
            object: davenda_auth::Entity::page("http.surface.module.cms.page.cms.preview"),
        }]
    );
}

#[tokio::test]
async fn server_host_authorizes_capability_routes_with_a_replacement_auth_package() {
    let config = config_with_auth_package("platform-extended-auth");
    let package = SelectedAuthModelPackage::new("platform-extended-auth", PackageMode::Extend);
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, package)
        .with_module(CmsModule::new())
        .with_template(fragment_template(customer_namespace, "cms/preview"))
        .build()
        .unwrap();
    assert_eq!(plan.auth_package_name, "platform-extended-auth");
    assert_eq!(plan.auth_package.manifest().mode, PackageMode::Extend);
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let authorizer = Arc::new(StaticLiveRouteCapabilityAuthorizer::new().allowing(
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user("editor-live-extend")),
        Capability::CmsPageRead,
        davenda_auth::Entity::page("http.surface.module.cms.page.cms.preview"),
    ));
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        cookie_secret.to_vec(),
        csrf_secret.to_vec(),
        authorizer.clone(),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("editor-live-extend")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/admin/pages/preview")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        authorizer.checks(),
        vec![LiveAuthorizationCheck {
            subject: davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(
                "editor-live-extend",
            )),
            capability: Capability::CmsPageRead,
            object: davenda_auth::Entity::page("http.surface.module.cms.page.cms.preview"),
        }]
    );
}

#[tokio::test]
async fn server_host_renders_page_templates_as_html() {
    let config = config_with_app_name("showcase-events-render-page");
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .with_template(page_template(customer_namespace, "account/dashboard"))
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-2")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/account")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    if status != StatusCode::OK {
        panic!("admin widget response failed: status={status}, body={body}");
    }
    assert_eq!(
        headers.get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    assert!(body.contains("<main data-route=\"account.dashboard\""));
    assert!(body.contains("/account"));
    assert!(body.contains("rel=\"canonical\""));
    assert!(body.contains("application/ld+json"));
    assert!(body.contains("\"@type\":\"WebPage\""));
    assert!(!body.contains("render:account/dashboard"));
}

#[tokio::test]
async fn server_host_loads_customer_storefront_templates_from_template_roots() {
    let config = config_with_app_name("harbor-shop-runtime-storefront");
    let template_root = unique_temp_template_root("storefront-pages");
    write_template_file(
        &template_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev" dv:with="pageTitle='Harbor Shop'">
  <head>
    <title dv:text="${pageTitle}">Harbor Shop</title>
  </head>
  <body>
    <header>
      <nav dv:replace="~{navigation/primary}"></nav>
    </header>
    <main class="storefront-home">
      <section>
        <h1 dv:text="${route_name}">Home</h1>
        <div dv:replace="~{commerce/collection-grid}"></div>
      </section>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/navigation/primary.html",
        r#"<nav class="primary-nav" xmlns:dv="https://davenda.dev" dv:fragment="primary">
  <a href="/collections">Collections</a>
  <a href="/account">Account</a>
</nav>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/collection-grid.html",
        r#"<section class="collection-grid" xmlns:dv="https://davenda.dev" dv:fragment="grid">
  <p>Featured collections load from customer templates.</p>
</section>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("primary-nav"), "{body}");
    assert!(
        body.contains("Featured collections load from customer templates."),
        "{body}"
    );
    assert!(body.contains("home"), "{body}");
}

#[tokio::test]
async fn server_host_loads_customer_account_templates_from_template_roots() {
    let config = config_with_app_name("harbor-shop-runtime-account");
    let template_root = unique_temp_template_root("account-pages");
    write_template_file(
        &template_root,
        "templates/account/dashboard.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <head>
    <title>Account</title>
  </head>
  <body>
    <section class="account-dashboard">
      <aside dv:replace="~{account/sidebar}"></aside>
      <main>
        <h1 dv:text="${route_name}">Account dashboard</h1>
        <p class="principal" dv:text="${principal_id}">member</p>
      </main>
    </section>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/account/sidebar.html",
        r#"<aside class="account-sidebar" xmlns:dv="https://davenda.dev" dv:fragment="sidebar">
  <a href="/account">Dashboard</a>
</aside>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_template_root(&template_root)
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session()
                .from_module("memberships"),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-template")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/account")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("account-sidebar"), "{body}");
    assert!(body.contains("account.dashboard"), "{body}");
    assert!(body.contains("member-live-template"), "{body}");
}

#[tokio::test]
async fn server_host_renders_checkout_confirmation_and_account_history_from_sample_order() {
    let app_name = unique_app_name("harbor-shop-runtime-order-flow");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("order-flow-pages");
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-page">
      <h1 dv:text="${page.title}">Checkout</h1>
      <p class="customer" dv:text="${customer.email}">customer@example.com</p>
      <ul class="line-items">
        <li dv:each="item : ${lineItems}">
          <span class="item-title" dv:text="${item.title}">Item</span>
          <span class="item-qty" dv:text="${item.quantity}">1</span>
          <strong class="item-total" dv:text="${item.total}">£0.00</strong>
        </li>
      </ul>
      <p class="grand-total" dv:text="${orderSummary.total}">£0.00</p>
      <p class="provider" dv:text="${checkout.providerLabel}">Provider</p>
      <p class="intent" dv:text="${checkout.paymentReference}">PAYMENT-PENDING</p>
      <form action="/checkout/complete" method="post">
        <button type="submit">Place order</button>
      </form>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout-confirmation.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-confirmation">
      <h1 dv:text="${confirmation.orderNumber}">ORD-10042</h1>
      <p class="confirmation-email" dv:text="${confirmation.email}">member@example.com</p>
      <p class="confirmation-status" dv:text="${confirmation.status}">Paid</p>
      <p class="confirmation-total" dv:text="${confirmation.total}">£118.00</p>
      <p class="confirmation-payment" dv:text="${confirmation.paymentSummary}">
        Card ending 4242, reference PAY-50001
      </p>
      <p class="confirmation-next-step" dv:text="${confirmation.nextStep}">
        A confirmation email and membership activation will follow shortly.
      </p>
      <div dv:replace="~{account/summary-panels :: panels}"></div>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/account/dashboard.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="account-dashboard">
      <h1 dv:text="${customer.displayName}">Account</h1>
      <p class="principal" dv:text="${principal_id}">member</p>
      <div dv:replace="~{account/summary-panels :: panels}"></div>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/account/orders.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="account-orders">
      <h1 dv:text="${customer.displayName}">Orders</h1>
      <p class="summary" dv:text="${account.stateSummary}">Summary</p>
      <ol class="orders">
        <li dv:each="order : ${recentOrders}">
          <strong dv:text="${order.reference}">ORD-10042</strong>
          <span dv:text="${order.status}">Paid</span>
          <span dv:text="${order.total}">£118.00</span>
          <span class="line-count" dv:text="${order.lineCount}">2</span>
          <span class="payment" dv:text="${order.paymentSummary}">Card ending 4242</span>
          <span class="email" dv:text="${order.checkoutEmail}">member@example.com</span>
        </li>
      </ol>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/account/summary-panels.html",
        r#"<section class="account-panels" xmlns:dv="https://davenda.dev" dv:fragment="panels">
  <div class="account-panels__grid">
    <article class="account-panel">
      <h2>Recent purchases</h2>
      <ul class="account-panel__list">
        <li dv:each="order : ${recentOrders}">
          <strong dv:text="${order.reference}">ORD-10042</strong>
          <span dv:text="${order.status}">Paid</span>
          <span dv:text="${order.total}">£118.00</span>
        </li>
      </ul>
    </article>
    <article class="account-panel">
      <h2>Membership</h2>
      <strong dv:text="${membershipSummary.tierName}">Harbor Circle</strong>
      <span dv:text="${membershipSummary.status}">Active</span>
      <p dv:text="${membershipSummary.renewalText}">Renews on 18 April</p>
    </article>
  </div>
</section>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_template_root(&template_root)
        .with_route(
            RouteDefinition::new("commerce.checkout", HttpMethod::Get, "/checkout")
                .unwrap()
                .from_module("commerce"),
        )
        .with_handler(HandlerDefinition::page("commerce.checkout", "commerce/checkout").unwrap())
        .with_route(
            RouteDefinition::new(
                "commerce.checkout-confirmation",
                HttpMethod::Get,
                "/checkout/confirmation",
            )
            .unwrap()
            .from_module("commerce"),
        )
        .with_handler(
            HandlerDefinition::page(
                "commerce.checkout-confirmation",
                "commerce/checkout-confirmation",
            )
            .unwrap(),
        )
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session()
                .from_module("memberships"),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .with_route(
            RouteDefinition::new(
                "commerce.account.orders",
                HttpMethod::Get,
                "/account/orders",
            )
            .unwrap()
            .with_area(RouteArea::Account)
            .requiring_session()
            .from_module("commerce"),
        )
        .with_handler(HandlerDefinition::page("commerce.account.orders", "account/orders").unwrap())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-order-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some("member-live-order-1"),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some("member-live-order-1"),
            "membership-gold",
            1,
            101,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some("member-live-order-1"), 102)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some("member-live-order-1"),
            &StorefrontPaymentInput::card("member-live-order-1@example.com", "4242", "PAY-50001")
                .unwrap(),
            103,
        )
        .unwrap();

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(checkout_body.contains("Harbor Cap"), "{checkout_body}");
    assert!(checkout_body.contains("Gold Membership"), "{checkout_body}");
    assert!(checkout_body.contains("£118.00"), "{checkout_body}");
    assert!(
        checkout_body.contains("Platform fallback payment path"),
        "{checkout_body}"
    );
    assert!(checkout_body.contains("PAY-50001"), "{checkout_body}");
    assert!(
        checkout_body.contains("/checkout/complete"),
        "{checkout_body}"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("ORD-10042"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("provider callback arrives"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Pending Payment"),
        "{confirmation_body}"
    );
    assert!(confirmation_body.contains("£118.00"), "{confirmation_body}");
    assert!(
        confirmation_body.contains("Card ending 4242, reference PAY-50001"),
        "{confirmation_body}"
    );

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    let order_history_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let order_history_content_type = response_header(&order_history_response, "content-type");
    let order_history_body = String::from_utf8(
        to_bytes(order_history_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    let order_history_json = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders.json")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("davenda_session={}", issued.cookie_value))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let json_content_type = response_header(&order_history_json, "content-type");
    let json_body = String::from_utf8(
        to_bytes(order_history_json.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert!(
        account_body.contains("member-live-order-1"),
        "{account_body}"
    );
    assert!(account_body.contains("ORD-10042"), "{account_body}");
    assert!(account_body.contains("Pending Payment"), "{account_body}");
    assert!(account_body.contains("£118.00"), "{account_body}");
    assert!(account_body.contains("Gold Membership"), "{account_body}");
    assert!(
        account_body.contains("Membership unavailable"),
        "{account_body}"
    );
    assert!(account_body.contains("Not active"), "{account_body}");
    assert_eq!(order_history_content_type, "text/html; charset=utf-8");
    assert!(
        order_history_body.contains("ORD-10042"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Card ending 4242, reference PAY-50001"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("member-live-order-1@example.com"),
        "{order_history_body}"
    );
    assert_eq!(json_content_type, "application/json");
    assert!(
        json_body.contains("\"order_id\":\"ORD-10042\""),
        "{json_body}"
    );
    assert!(json_body.contains("\"sku\":\"harbor-cap\""), "{json_body}");
    assert!(
        json_body.contains("\"reference\":\"PAY-50001\""),
        "{json_body}"
    );
    assert!(
        json_body.contains("\"checkout_email\":\"member-live-order-1@example.com\""),
        "{json_body}"
    );
}

#[tokio::test]
async fn server_host_bootstraps_guest_storefront_session_and_injects_live_state() {
    let app_name = unique_app_name("harbor-shop-runtime-storefront-state");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("storefront-state-pages");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="cart-page">
      <h1 dv:text="${route_name}">Cart</h1>
      <p class="empty" dv:unless="${hasCartItems}">Your cart is empty.</p>
      <ul class="cart-lines">
        <li dv:each="item : ${cartItems}" dv:text="${item.title}">Fallback item</li>
      </ul>
      <p class="subtotal" dv:text="${cartSummary.subtotal}">£0.00</p>
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let cart_items = response_header(&response, "x-davenda-storefront-cart-items");
    let add_to_cart_token =
        response_header(&response, "x-davenda-storefront-csrf-commerce-add-to-cart");
    let session_cookie = response_session_cookie(&response);
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(cart_items, "0");
    assert!(!add_to_cart_token.is_empty());
    assert!(!session_cookie.is_empty());
    assert!(body.contains("davenda-storefront-state"), "{body}");
    assert!(body.contains("\"route\":\"commerce.cart\""), "{body}");
    assert!(body.contains("\"item_count\":0"), "{body}");
    assert!(body.contains("Your cart is empty."), "{body}");
    assert!(body.contains("£0.00"), "{body}");
    assert!(!body.contains("Harbor Cap"), "{body}");
}

#[tokio::test]
async fn server_host_executes_storefront_add_to_cart_checkout_and_confirmation_flow() {
    let app_name = unique_app_name("harbor-shop-runtime-native-storefront");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("native-storefront-flow");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="cart-page">
      <ul class="cart-lines">
        <li dv:each="item : ${cartItems}">
          <span class="item-title" dv:text="${item.title}">Item</span>
          <span class="item-qty" dv:text="${item.quantity}">1</span>
          <strong class="item-total" dv:text="${item.total}">£0.00</strong>
        </li>
      </ul>
      <p class="cart-subtotal" dv:text="${cartSummary.subtotal}">£0.00</p>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-page">
      <ul class="line-items">
        <li dv:each="item : ${lineItems}">
          <span class="item-title" dv:text="${item.title}">Item</span>
          <span class="item-qty" dv:text="${item.quantity}">1</span>
          <strong class="item-total" dv:text="${item.total}">£0.00</strong>
        </li>
      </ul>
      <p class="checkout-total" dv:text="${orderSummary.total}">£0.00</p>
      <p class="checkout-provider" dv:text="${checkout.providerLabel}">Provider</p>
      <p class="checkout-reference" dv:text="${checkout.paymentReference}">PAYMENT-PENDING</p>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout-confirmation.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-confirmation">
      <h1 class="order-number" dv:text="${confirmation.orderNumber}">ORD-10042</h1>
      <p class="payment-summary" dv:text="${confirmation.paymentSummary}">
        Card ending 4242, reference PAY-50001
      </p>
      <p class="order-total" dv:text="${confirmation.total}">£0.00</p>
      <ul class="confirmation-lines">
        <li dv:each="item : ${confirmation.lineItems}">
          <span class="item-title" dv:text="${item.title}">Item</span>
          <span class="item-qty" dv:text="${item.quantity}">1</span>
          <strong class="item-total" dv:text="${item.total}">£0.00</strong>
        </li>
      </ul>
      <p class="next-step" dv:text="${confirmation.nextStep}">Next step</p>
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sku", "harbor-cap")
        .append_pair("quantity", "2")
        .finish();
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(add_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(add_response.headers().get("location").unwrap(), "/cart");

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let cart_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(cart_body.contains("Harbor Cap"), "{cart_body}");
    assert!(cart_body.contains("2"), "{cart_body}");
    assert!(cart_body.contains("£58.00"), "{cart_body}");

    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        checkout_start.headers().get("location").unwrap(),
        "/checkout"
    );

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(checkout_body.contains("Harbor Cap"), "{checkout_body}");
    assert!(checkout_body.contains("£58.00"), "{checkout_body}");
    assert!(
        checkout_body.contains("Platform fallback payment path"),
        "{checkout_body}"
    );
    assert!(checkout_body.contains("PAY-50001"), "{checkout_body}");

    let complete_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("checkout_email", "buyer@example.com")
        .append_pair("payment_method", "card")
        .append_pair("payment_last4", "4242")
        .append_pair("checkout_intent", "PAY-50001")
        .append_pair("terms_accepted", "yes")
        .finish();
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(complete_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        complete_response.headers().get("location").unwrap(),
        "/checkout/confirmation"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let payment_status = response_header(
        &confirmation_response,
        "x-davenda-storefront-payment-status",
    );
    let payment_reference = response_header(
        &confirmation_response,
        "x-davenda-storefront-payment-reference",
    );
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert!(
        confirmation_body.contains("ORD-10042"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Harbor Cap"),
        "{confirmation_body}"
    );
    assert!(confirmation_body.contains("2"), "{confirmation_body}");
    assert!(
        confirmation_body.contains("provider callback arrives"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Card ending 4242, reference PAY-50001"),
        "{confirmation_body}"
    );
    assert!(confirmation_body.contains("£58.00"), "{confirmation_body}");
    assert!(
        confirmation_body.contains("davenda-storefront-state"),
        "{confirmation_body}"
    );
    assert_eq!(payment_status, "provider_pending");
    assert_eq!(payment_reference, "PAY-50001");
    assert!(
        confirmation_body.contains("\"status\":\"provider_pending\""),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("\"reference\":\"PAY-50001\""),
        "{confirmation_body}"
    );

    let order_history = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders.json")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let history_body = String::from_utf8(
        to_bytes(order_history.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        history_body.contains("\"order_id\":\"ORD-10042\""),
        "{history_body}"
    );
    assert!(
        history_body.contains("\"sku\":\"harbor-cap\""),
        "{history_body}"
    );
    assert!(
        history_body.contains("\"reference\":\"PAY-50001\""),
        "{history_body}"
    );
    assert!(
        history_body.contains("\"checkout_email\":\"buyer@example.com\""),
        "{history_body}"
    );
}

#[tokio::test]
async fn server_host_rejects_checkout_completion_without_payment_details() {
    let app_name = unique_app_name("harbor-shop-runtime-checkout-payment-required");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("native-storefront-payment-required");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="cart-page">
      <h1>Cart</h1>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-page">
      <p class="checkout-summary" dv:if="${checkout.hasErrors}" dv:text="${checkout.errorSummary}">Summary</p>
      <p class="email-error" dv:if="${checkout.hasCheckoutEmailError}" dv:text="${checkout.checkoutEmailError}">Email error</p>
      <p class="last4-error" dv:if="${checkout.hasPaymentLast4Error}" dv:text="${checkout.paymentLast4Error}">Last4 error</p>
      <p class="terms-error" dv:if="${checkout.hasTermsAcceptedError}" dv:text="${checkout.termsAcceptedError}">Terms error</p>
      <input class="checkout-email" type="email" dv:attr="value=${checkout.checkoutEmail}" />
      <input class="payment-method" type="text" dv:attr="value=${checkout.paymentMethod}" />
      <input class="payment-last4" type="text" dv:attr="value=${checkout.paymentLast4}" />
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout-confirmation.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="confirmation-page">
      <h1>Confirmation</h1>
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("sku", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = complete_response.status();
    let location = response_header(&complete_response, "location");
    let flash_cookie =
        cookie_pair_from_response(&complete_response, "davenda_flash").expect("flash cookie");

    let checkout_retry_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("{session_cookie}; {flash_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = String::from_utf8(
        to_bytes(checkout_retry_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(location, "/checkout");
    assert!(
        body.contains("There is a problem with your checkout details."),
        "{body}"
    );
    assert!(
        body.contains("Enter the final 4 digits for the payment card."),
        "{body}"
    );
    assert!(
        body.contains("Review the basket and confirm the final total before placing the order."),
        "{body}"
    );
    assert!(body.contains("value=\"buyer@example.com\""), "{body}");
    assert!(body.contains("value=\"card\""), "{body}");
}

#[tokio::test]
async fn server_host_rejects_checkout_completion_without_reserved_payment_intent() {
    let app_name = unique_app_name("harbor-shop-runtime-checkout-intent-required");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("native-storefront-intent-required");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="cart-page">
      <h1>Cart</h1>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-page">
      <p class="checkout-summary" dv:if="${checkout.hasErrors}" dv:text="${checkout.errorSummary}">Summary</p>
      <p class="intent-error" dv:if="${checkout.hasCheckoutIntentError}" dv:text="${checkout.checkoutIntentError}">Intent error</p>
      <input class="checkout-email" type="email" dv:attr="value=${checkout.checkoutEmail}" />
      <input class="payment-last4" type="text" dv:attr="value=${checkout.paymentLast4}" />
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout-confirmation.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="confirmation-page">
      <h1>Confirmation</h1>
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(&issued.record.session_id, None, "harbor-cap", 1, 100)
        .unwrap();
    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("sku", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("payment_last4", "4242")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = complete_response.status();
    let location = response_header(&complete_response, "location");
    let flash_cookie =
        cookie_pair_from_response(&complete_response, "davenda_flash").expect("flash cookie");
    let checkout_retry_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("{session_cookie}; {flash_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = String::from_utf8(
        to_bytes(checkout_retry_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(location, "/checkout");
    assert!(
        body.contains("Refresh checkout before placing the order."),
        "{body}"
    );
    assert!(
        body.contains("Refresh checkout and try again before placing the order."),
        "{body}"
    );
    assert!(body.contains("value=\"buyer@example.com\""), "{body}");
    assert!(body.contains("value=\"4242\""), "{body}");
}

#[tokio::test]
async fn server_host_redirects_cart_validation_failures_back_to_cart_with_repopulated_lines() {
    let app_name = unique_app_name("harbor-shop-runtime-cart-validation-prg");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("native-storefront-cart-validation");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="cart-page">
      <p class="cart-summary" dv:if="${cartForm.hasErrors}" dv:text="${cartForm.errorSummary}">Summary</p>
      <ul class="cart-lines">
        <li dv:each="item : ${cartItems}">
          <input class="item-qty" type="number" dv:attr="name=${item.quantityField},value=${item.quantity}" />
          <p class="item-error" dv:if="${item.hasQuantityError}" dv:text="${item.quantityError}">Error</p>
        </li>
      </ul>
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            None,
            "harbor-cap",
            1,
            now.as_unix_seconds(),
        )
        .unwrap();

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cart_response_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let cart_update_token =
        storefront_csrf_token_from_body(&cart_response_body, "commerce.cart-update");

    let update_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", cart_update_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("quantity_harbor-cap", "abc")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let flash_cookie =
        cookie_pair_from_response(&update_response, "davenda_flash").expect("flash cookie");
    assert_eq!(update_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&update_response, "location"), "/cart");

    let cart_retry = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("{session_cookie}; {flash_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = String::from_utf8(
        to_bytes(cart_retry.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert!(
        body.contains("Fix the highlighted cart quantities and try again."),
        "{body}"
    );
    assert!(
        body.contains("Enter a whole-number quantity for this line."),
        "{body}"
    );
    assert!(body.contains("value=\"abc\""), "{body}");
}

#[tokio::test]
async fn server_host_renders_checkout_form_defaults_for_active_checkout() {
    let app_name = unique_app_name("harbor-shop-runtime-checkout-defaults");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("native-storefront-checkout-defaults");
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-page">
      <input class="checkout-email" type="email" dv:attr="value=${checkout.checkoutEmail}" />
      <input class="payment-method" type="text" dv:attr="value=${checkout.paymentMethod}" />
      <input class="payment-last4" type="text" dv:attr="value=${checkout.paymentLast4}" />
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(&issued.record.session_id, None, "harbor-cap", 1, 100)
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, None, 101)
        .unwrap();

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert!(checkout_body.contains("value=\"card\""), "{checkout_body}");
    assert!(checkout_body.contains("value=\"\""), "{checkout_body}");
}

#[tokio::test]
async fn server_host_rejects_payment_webhooks_with_invalid_signatures() {
    let app_name = unique_app_name("harbor-shop-runtime-invalid-payment-webhook");
    let config = with_payment_webhook_secret(config_with_app_name(&app_name));
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("provider", "stripe")
                        .append_pair("event", "payment.captured")
                        .append_pair("payment_reference", "PAY-50001")
                        .append_pair("signature", "not-valid")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("payment webhook verification failed"),
        "{body}"
    );
}

#[tokio::test]
async fn server_host_rejects_native_stripe_webhooks_with_invalid_stripe_signatures() {
    let app_name = unique_app_name("harbor-shop-runtime-invalid-native-stripe-webhook");
    let config = with_stripe_payment_provider(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let payload = serde_json::json!({
        "id": "evt_test_invalid",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": "cs_test_invalid",
                "client_reference_id": "PAY-50001",
                "metadata": {
                    "payment_reference": "PAY-50001"
                }
            }
        }
    })
    .to_string();
    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/json")
                .header("stripe-signature", "t=1712345678,v1=not-valid")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("payment webhook verification failed"),
        "{body}"
    );
}

#[tokio::test]
async fn server_host_rejects_native_stripe_webhooks_with_stale_timestamps() {
    let app_name = unique_app_name("harbor-shop-runtime-stale-native-stripe-webhook");
    let config = with_stripe_payment_provider(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let payload = serde_json::json!({
        "id": "evt_test_stale",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": "cs_test_stale",
                "client_reference_id": "PAY-50001",
                "metadata": {
                    "payment_reference": "PAY-50001"
                }
            }
        }
    })
    .to_string();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/json")
                .header(
                    "stripe-signature",
                    stripe_webhook_signature(now - 600, &payload),
                )
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("payment webhook verification failed"),
        "{body}"
    );
}

#[tokio::test]
async fn server_host_rejects_payment_webhooks_for_an_unconfigured_provider() {
    let app_name = unique_app_name("harbor-shop-runtime-payment-provider-mismatch");
    let config = with_stripe_payment_provider(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("provider", "paypal")
                        .append_pair("event", "payment.captured")
                        .append_pair("payment_reference", "PAY-50001")
                        .append_pair(
                            "signature",
                            &payment_webhook_signature("paypal", "payment.captured", "PAY-50001"),
                        )
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body.contains("does not match configured provider `stripe`"),
        "{body}"
    );
}

#[tokio::test]
async fn server_host_restores_checkout_after_payment_failure_webhook() {
    let app_name = unique_app_name("harbor-shop-runtime-payment-failure-recovery");
    let config = with_payment_webhook_secret(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "checkout-failure-member";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.failed")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.failed", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(webhook_response.status(), StatusCode::OK);

    let snapshot = store
        .snapshot(&issued.record.session_id, Some(principal_id))
        .unwrap();
    assert_eq!(snapshot.cart.status, "active");
    assert_eq!(snapshot.payment.status, "failed");
    assert_eq!(snapshot.cart.item_count, 1);
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.payment.status.as_str()),
        Some("failed")
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_status = confirmation_response.status();
    let confirmation_location = response_header(&confirmation_response, "location");
    let flash_cookie =
        cookie_pair_from_response(&confirmation_response, "davenda_flash").expect("flash cookie");
    assert_eq!(confirmation_status, StatusCode::SEE_OTHER);
    assert_eq!(confirmation_location, "/cart");

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("{session_cookie}; {flash_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cart_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        cart_body.contains(
            "Payment for order ORD-10042 failed. Your basket has been restored so you can review it and start checkout again."
        ),
        "{cart_body}"
    );
    assert!(cart_body.contains("Harbor Cap"), "{cart_body}");
    assert!(
        cart_body.contains("/en-GB/shop/products/harbor-cap"),
        "{cart_body}"
    );
    assert!(
        cart_body.contains("/en-GB/shop/collections/featured"),
        "{cart_body}"
    );
    assert!(cart_body.contains("/en-GB/shop/collections"), "{cart_body}");
    assert!(cart_body.contains("Checkout"), "{cart_body}");
}

#[tokio::test]
async fn server_host_renders_checked_in_harbor_shop_stripe_checkout_contract() {
    let app_name = unique_app_name("harbor-shop-runtime-stripe-checkout-contract");
    let mut config = checked_in_harbor_shop_config(&app_name);
    config.auth.package = "platform-default-auth".to_string();
    let template_root = unique_temp_template_root("stripe-checkout-contract");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main class="cart-page"><h1>Cart</h1></main></body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-page">
      <p class="provider" dv:text="${checkout.providerLabel}">Provider</p>
      <p class="summary" dv:text="${checkout.providerSummary}">Summary</p>
      <button class="submit" dv:text="${checkout.submitLabel}">Submit</button>
      <p class="reference" dv:text="${checkout.paymentReference}">PAY-50001</p>
    </main>
  </body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout-confirmation.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="confirmation-page">
      <div class="flash" dv:if="${hasFlashMessages}">
        <p dv:each="message : ${flashMessages}" dv:text="${message.text}">Flash</p>
      </div>
      <p class="provider" dv:text="${confirmation.providerLabel}">Provider</p>
      <p class="next-step" dv:text="${confirmation.nextStep}">Next step</p>
    </main>
  </body>
</html>"#,
    );
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_contract",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let session_cookie =
        cookie_pair_from_response(&cart_bootstrap, "davenda_session").expect("session cookie");
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        checkout_body.contains("Stripe hosted checkout"),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains(
            "This checkout reserves the order in Davenda, then redirects the customer to Stripe Checkout for payment collection. Davenda still waits for the signed Stripe webhook before treating the order as paid."
        ),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains("Continue to Stripe"),
        "{checkout_body}"
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("payment_last4", "4242")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "https://checkout.stripe.test/session/cs_test_harbor_shop_contract"
    );
    let checkout_calls = checkout_client.take_calls();
    assert_eq!(checkout_calls.len(), 1);
    assert_eq!(checkout_calls[0].api_key, STRIPE_SECRET_KEY);
    assert_eq!(checkout_calls[0].idempotency_key, "davenda-order-ORD-10042");
    assert!(
        checkout_calls[0]
            .request_body
            .contains("client_reference_id=PAY-50001"),
        "{:?}",
        checkout_calls[0]
    );
    assert!(
        checkout_calls[0]
            .request_body
            .contains("payment_intent_data%5Bmetadata%5D%5Border_id%5D=ORD-10042"),
        "{:?}",
        checkout_calls[0]
    );
    assert!(
        checkout_calls[0]
            .request_body
            .contains("customer_email=buyer%40example.com"),
        "{:?}",
        checkout_calls[0]
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation?checkout_session_id=cs_test_harbor_shop_contract")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert!(
        confirmation_body.contains(
            "Stripe Checkout has not confirmed this payment yet. The order will move forward after the hosted Stripe session finishes and the signed Stripe webhook arrives."
        ),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Stripe hosted checkout"),
        "{confirmation_body}"
    );

    fs::remove_dir_all(&template_root).unwrap();
}

#[tokio::test]
async fn server_host_ignores_regressive_payment_failure_after_capture() {
    let app_name = unique_app_name("harbor-shop-runtime-payment-regression");
    let config = with_payment_webhook_secret(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "checkout-paid-member";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    for event in ["payment.captured", "payment.failed"] {
        let webhook_body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("provider", "stripe")
            .append_pair("event", event)
            .append_pair("payment_reference", "PAY-50001")
            .append_pair(
                "signature",
                &payment_webhook_signature("stripe", event, "PAY-50001"),
            )
            .finish();
        let webhook_response = server
            .respond(
                Request::builder()
                    .method("POST")
                    .uri("/webhooks/commerce/payment-provider")
                    .header("host", "www.example.com")
                    .header("x-forwarded-proto", "https")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(webhook_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(webhook_response.status(), StatusCode::OK);
    }

    let snapshot = store
        .snapshot(&issued.record.session_id, Some(principal_id))
        .unwrap();
    assert_eq!(snapshot.cart.item_count, 0);
    assert_eq!(snapshot.payment.status, "captured");
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.status.as_str()),
        Some("paid")
    );
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.payment.status.as_str()),
        Some("captured")
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("Status <strong>Paid</strong>"),
        "{confirmation_body}"
    );
    assert!(
        !confirmation_body.contains("Payment for order ORD-10042 failed."),
        "{confirmation_body}"
    );
}

#[tokio::test]
async fn server_host_accepts_checkout_completion_with_card_last4_only() {
    let app_name = unique_app_name("harbor-shop-runtime-checkout-card-last4");
    let config = config_with_app_name(&app_name);
    let template_root = unique_temp_template_root("native-storefront-card-last4");
    write_template_file(
        &template_root,
        "templates/commerce/cart.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main class="cart-page"><h1>Cart</h1></main></body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main class="checkout-page"><h1>Checkout</h1></main></body>
</html>"#,
    );
    write_template_file(
        &template_root,
        "templates/commerce/checkout-confirmation.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body>
    <main class="checkout-confirmation">
      <h1 class="order-number" dv:text="${confirmation.orderNumber}">ORD-10042</h1>
    </main>
  </body>
</html>"#,
    );

    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let session_cookie =
        cookie_pair_from_response(&cart_bootstrap, "davenda_session").expect("session cookie");
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("card_last4", "4242")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        complete_response.headers().get("location").unwrap(),
        "/checkout/confirmation"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert!(
        confirmation_body.contains("ORD-10042"),
        "{confirmation_body}"
    );
}

#[tokio::test]
async fn server_host_executes_checked_in_harbor_shop_membership_storefront_flow() {
    let app_name = unique_app_name("harbor-shop-runtime-checked-in-storefront");
    let config = with_payment_webhook_secret(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-checkedin";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "gold-membership",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("member@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_status = account_response.status();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(account_status, StatusCode::OK, "{account_body}");
    assert!(account_body.contains("Latest order"), "{account_body}");
    assert!(account_body.contains("Pending Payment"), "{account_body}");
    assert!(
        account_body.contains("Membership access moves into this account area"),
        "{account_body}"
    );
    assert!(account_body.contains("View memberships"), "{account_body}");
    assert!(
        account_body.contains("View order history"),
        "{account_body}"
    );

    let memberships_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let memberships_body = String::from_utf8(
        to_bytes(memberships_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        memberships_body.contains("Gold Membership"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("Pending activation"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("Latest order"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("Included with order ORD-10042."),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("Pending Payment"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("member@example.com"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("View order history"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("View membership details"),
        "{memberships_body}"
    );

    let order_history_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let order_history_body = String::from_utf8(
        to_bytes(order_history_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        order_history_body.contains("member@example.com"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("ORD-10042"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Card ending 4242, reference PAY-50001"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("confirm the latest status, then return to memberships"),
        "{order_history_body}"
    );

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("x-stripe-delivery", "evt_test_verified_hook")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let webhook_status = webhook_response.status();
    let webhook_response_body = String::from_utf8(
        to_bytes(webhook_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(webhook_status, StatusCode::OK, "{webhook_response_body}");
    assert!(
        webhook_response_body.contains("\"status\":\"accepted\""),
        "{webhook_response_body}"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("Status <strong>Paid</strong>"),
        "{confirmation_body}"
    );
    assert!(
        !confirmation_body.contains("Payment confirmation is pending."),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Card ending 4242, reference PAY-50001"),
        "{confirmation_body}"
    );

    let activated_memberships_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let activated_memberships_body = String::from_utf8(
        to_bytes(activated_memberships_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        activated_memberships_body.contains("Gold Membership"),
        "{activated_memberships_body}"
    );
    assert!(
        activated_memberships_body.contains("Active"),
        "{activated_memberships_body}"
    );
    assert!(
        activated_memberships_body.contains("Activated from order ORD-10042."),
        "{activated_memberships_body}"
    );
    assert!(
        !activated_memberships_body.contains("Pending activation"),
        "{activated_memberships_body}"
    );
}

#[tokio::test]
async fn server_host_bootstraps_checked_in_harbor_shop_account_entry_without_sign_in() {
    let app_name = unique_app_name("harbor-shop-runtime-account-entry");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_cookie =
        cookie_pair_from_response(&account_response, "davenda_session").expect("session cookie");
    let account_status = account_response.status();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(account_status, StatusCode::OK, "{account_body}");
    assert!(account_body.contains("Your account"), "{account_body}");
    assert!(
        account_body.contains("This account area follows the current browser session"),
        "{account_body}"
    );
    assert!(account_body.contains("Account overview"), "{account_body}");
    assert!(account_body.contains("Order history"), "{account_body}");
    assert!(account_body.contains("Memberships"), "{account_body}");
    assert!(
        account_body.contains("End browser session"),
        "{account_body}"
    );
    assert!(account_body.contains("Open checkout"), "{account_body}");
    assert!(account_body.contains("Continue shopping"), "{account_body}");
    assert!(
        account_body.contains("Explore memberships"),
        "{account_body}"
    );

    let order_history_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &account_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let order_history_status = order_history_response.status();
    let order_history_body = String::from_utf8(
        to_bytes(order_history_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(order_history_status, StatusCode::OK, "{order_history_body}");
    assert!(
        order_history_body.contains("Order history"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("This order history currently follows the browser session"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("No completed orders yet"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Browse storefront"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Open checkout"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Continue shopping"),
        "{order_history_body}"
    );

    let memberships_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &account_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let memberships_status = memberships_response.status();
    let memberships_body = String::from_utf8(
        to_bytes(memberships_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(memberships_status, StatusCode::OK, "{memberships_body}");
    assert!(
        memberships_body.contains("Memberships"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("currently follows the browser session"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("Membership not active yet"),
        "{memberships_body}"
    );
    assert!(
        memberships_body.contains("Explore memberships"),
        "{memberships_body}"
    );
}

#[tokio::test]
async fn server_host_can_end_a_checked_in_harbor_shop_account_session() {
    let app_name = unique_app_name("harbor-shop-runtime-account-session-end");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let session_cookie =
        cookie_pair_from_response(&account_response, "davenda_session").expect("session cookie");
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let session_end_token =
        storefront_csrf_token_from_body(&account_body, "commerce.account-session-end");
    assert!(
        account_body.contains("davenda-account-session-end"),
        "{account_body}"
    );

    let end_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/account/session/end")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", session_end_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let end_status = end_response.status();
    let end_location = response_header(&end_response, "location");
    let cleared_session_cookie = end_response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| value.starts_with("davenda_session=") && value.contains("Max-Age=0"));
    let flash_cookie =
        cookie_pair_from_response(&end_response, "davenda_flash").expect("flash cookie");
    assert_eq!(end_status, StatusCode::SEE_OTHER);
    assert_eq!(end_location, "/account");
    assert!(cleared_session_cookie);

    let redirected_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &flash_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let redirected_status = redirected_response.status();
    let renewed_session_cookie = cookie_pair_from_response(&redirected_response, "davenda_session")
        .expect("renewed session cookie");
    let redirected_body = String::from_utf8(
        to_bytes(redirected_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(redirected_status, StatusCode::OK, "{redirected_body}");
    assert_ne!(renewed_session_cookie, session_cookie);
    assert!(
        redirected_body
            .contains("Account session ended. Start again from this browser when you are ready."),
        "{redirected_body}"
    );
    assert!(
        redirected_body.contains("This account currently follows the browser session"),
        "{redirected_body}"
    );
}

#[tokio::test]
async fn server_host_renders_checked_in_harbor_shop_catalog_collection_and_product_routes() {
    let app_name = unique_app_name("harbor-shop-runtime-catalog-routes");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let home_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let home_status = home_response.status();
    let home_body = String::from_utf8(
        to_bytes(home_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(home_status, StatusCode::OK, "{home_body}");
    assert!(home_body.contains("Browse collections"), "{home_body}");
    assert!(home_body.contains("/en-GB/shop/collections"), "{home_body}");
    assert!(!home_body.contains("href=\"/collections\""), "{home_body}");

    let collections_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collections_status = collections_response.status();
    let collections_body = String::from_utf8(
        to_bytes(collections_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(collections_status, StatusCode::OK, "{collections_body}");
    assert!(
        collections_body.contains("Shop Collections"),
        "{collections_body}"
    );
    assert!(
        collections_body.contains("/en-GB/shop/collections/featured"),
        "{collections_body}"
    );
    assert!(
        collections_body.contains("Gold Membership"),
        "{collections_body}"
    );

    let collection_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collection_status = collection_response.status();
    let collection_body = String::from_utf8(
        to_bytes(collection_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(collection_status, StatusCode::OK, "{collection_body}");
    assert!(
        collection_body.contains("Gold Membership"),
        "{collection_body}"
    );
    assert!(
        collection_body.contains("/en-GB/shop/collections"),
        "{collection_body}"
    );
    assert!(!collection_body.contains("Harbor Cap"), "{collection_body}");

    let product_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/gold-membership")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let product_status = product_response.status();
    let product_body = String::from_utf8(
        to_bytes(product_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(product_status, StatusCode::OK, "{product_body}");
    assert!(product_body.contains("Gold Membership"), "{product_body}");
    assert!(
        product_body.contains("value=\"gold-membership\""),
        "{product_body}"
    );
    assert!(
        product_body.contains("/en-GB/shop/collections/memberships"),
        "{product_body}"
    );
    assert!(!product_body.contains("Harbor Cap"), "{product_body}");
}

#[test]
fn runtime_plan_registers_checked_in_harbor_shop_root_route_from_customer_home_template() {
    let app_name = unique_app_name("harbor-shop-runtime-root-route");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();

    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/").unwrap(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    assert_eq!(execution.route.route_name, "home");
    assert_eq!(execution.path, "/");
    assert_eq!(execution.locale, "en-GB");
    match execution.response {
        HandlerResponse::Page(page) => assert_eq!(page.template, "pages/home"),
        other => panic!("expected page response for synthesized home route, got {other:?}"),
    }
}

#[tokio::test]
async fn server_host_injects_hidden_csrf_inputs_into_checked_in_storefront_forms() {
    let app_name = unique_app_name("harbor-shop-runtime-storefront-form-csrf");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let product_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/gold-membership")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let product_body = String::from_utf8(
        to_bytes(product_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let add_token = storefront_csrf_token_from_body(&product_body, "commerce.add-to-cart");
    assert!(
        product_body.contains(&format!(r#"name="_csrf" value="{add_token}""#)),
        "{product_body}"
    );

    let add_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &add_token)
        .append_pair("product_slug", "gold-membership")
        .append_pair("quantity", "1")
        .finish();
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(add_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&add_response, "location"), "/cart");

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cart_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let cart_update_token = storefront_csrf_token_from_body(&cart_body, "commerce.cart-update");
    let checkout_start_token =
        storefront_csrf_token_from_body(&cart_body, "commerce.checkout-start");
    assert!(
        cart_body.contains(&format!(r#"name="_csrf" value="{cart_update_token}""#)),
        "{cart_body}"
    );
    assert!(
        cart_body.contains(&format!(r#"name="_csrf" value="{checkout_start_token}""#)),
        "{cart_body}"
    );

    let checkout_start_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &checkout_start_token)
        .finish();
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(checkout_start_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let checkout_complete_token =
        storefront_csrf_token_from_body(&checkout_body, "commerce.checkout-complete");
    assert!(
        checkout_body.contains(&format!(
            r#"name="_csrf" value="{checkout_complete_token}""#
        )),
        "{checkout_body}"
    );
}

#[tokio::test]
async fn server_host_executes_checked_in_harbor_shop_customer_and_operator_journey() {
    let app_name = unique_app_name("harbor-shop-runtime-customer-operator-journey");
    let config = with_payment_webhook_secret(config_with_app_name(&app_name));
    let template_root = checked_in_harbor_shop_root();
    let mut config = config;
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan.clone(),
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-customer-operator";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let home_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let home_body = String::from_utf8(
        to_bytes(home_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(home_body.contains("Harbor Shop"), true, "{home_body}");
    assert!(home_body.contains("/en-GB/shop/collections"), "{home_body}");
    assert!(home_body.contains("/account"), "{home_body}");

    let collections_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collections_body = String::from_utf8(
        to_bytes(collections_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        collections_body.contains("Gold Membership"),
        "{collections_body}"
    );
    assert!(
        collections_body.contains("/en-GB/shop/products/gold-membership"),
        "{collections_body}"
    );

    let product_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/gold-membership")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let product_body = String::from_utf8(
        to_bytes(product_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(product_body.contains("Gold Membership"), "{product_body}");
    assert!(
        product_body.contains("value=\"gold-membership\""),
        "{product_body}"
    );
    assert!(product_body.contains("Add to cart"), "{product_body}");

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&add_response, "location"), "/cart");

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let cart_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(cart_body.contains("Gold Membership"), "{cart_body}");
    assert!(cart_body.contains("£89.00"), "{cart_body}");
    assert!(cart_body.contains("/checkout/start"), "{cart_body}");

    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(checkout_body.contains("Gold Membership"), "{checkout_body}");
    assert!(checkout_body.contains("PAY-50001"), "{checkout_body}");
    assert!(checkout_body.contains("Intent"), "{checkout_body}");

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("payment_last4", "4242")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "/checkout/confirmation"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("ORD-10042"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("provider callback arrives"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Pending Payment"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Card ending 4242, reference PAY-50001"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Gold Membership"),
        "{confirmation_body}"
    );

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        account_body.contains("Membership access moves into this account area"),
        "{account_body}"
    );
    assert!(account_body.contains("Pending Payment"), "{account_body}");
    assert!(
        account_body.contains("returned from the payment provider"),
        "{account_body}"
    );
    assert!(account_body.contains("buyer@example.com"), "{account_body}");

    let account_orders_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_orders_body = String::from_utf8(
        to_bytes(account_orders_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        account_orders_body.contains("ORD-10042"),
        "{account_orders_body}"
    );
    assert!(
        account_orders_body.contains("Card ending 4242, reference PAY-50001"),
        "{account_orders_body}"
    );
    assert!(
        account_orders_body.contains("Gold Membership"),
        "{account_orders_body}"
    );
    assert!(
        account_orders_body.contains("Pending Payment"),
        "{account_orders_body}"
    );
    assert!(
        account_orders_body.contains("same browser session"),
        "{account_orders_body}"
    );
    assert!(
        account_orders_body.contains("buyer@example.com"),
        "{account_orders_body}"
    );

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_body = String::from_utf8(
        to_bytes(admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(admin_body.contains("Harbor Shop Admin"), "{admin_body}");
    assert!(admin_body.contains("operator review"), "{admin_body}");

    let admin_orders_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_orders_body = String::from_utf8(
        to_bytes(admin_orders_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        admin_orders_body.contains("ORD-10042"),
        "{admin_orders_body}"
    );
    assert!(
        admin_orders_body.contains("View details"),
        "{admin_orders_body}"
    );
    assert!(admin_orders_body.contains("£89.00"), "{admin_orders_body}");
    assert!(
        admin_orders_body.contains("Pending Payment"),
        "{admin_orders_body}"
    );
    assert!(
        admin_orders_body.contains("buyer@example.com"),
        "{admin_orders_body}"
    );
    assert!(
        admin_orders_body.contains("store-wide"),
        "{admin_orders_body}"
    );
}

#[tokio::test]
async fn server_host_runs_sdk_checkout_hooks_before_stripe_handoff() {
    let app_name = unique_app_name("harbor-shop-runtime-checkout-hooks");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RejectMembershipCheckoutPlugin)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_blocked",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-checkout-hooks";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let flash_cookie =
        cookie_pair_from_response(&complete_response, "davenda_flash").expect("flash cookie");
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&complete_response, "location"), "/checkout");
    assert!(checkout_client.take_calls().is_empty());

    let checkout_retry = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("{session_cookie}; {flash_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_retry_body = String::from_utf8(
        to_bytes(checkout_retry.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert!(
        checkout_retry_body.contains(
            "Orders containing Gold Membership require manual review before payment can start."
        ),
        "{checkout_retry_body}"
    );
    assert!(
        checkout_retry_body.contains("buyer@example.com"),
        "{checkout_retry_body}"
    );
}

#[tokio::test]
async fn server_host_preserves_checkout_line_metadata_for_live_customer_hooks() {
    let app_name = unique_app_name("harbor-shop-runtime-checkout-line-metadata");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingCheckoutLineMetadataPlugin {
            calls: calls.clone(),
        })
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_line_metadata",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client,
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-checkout-line-metadata";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);

    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].sku, "harbor-cap");
    assert_eq!(
        calls[0].metadata.get("variant_title").map(String::as_str),
        Some("Standard")
    );
    assert_eq!(
        calls[0]
            .metadata
            .get("collection_handle")
            .map(String::as_str),
        Some("featured")
    );
}

#[tokio::test]
async fn server_host_runs_sdk_verified_webhook_hooks_in_live_payment_webhook_flow() {
    let app_name = unique_app_name("harbor-shop-runtime-verified-webhook-hooks");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingVerifiedWebhookPlugin {
            calls: calls.clone(),
        })
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_verified_hooks",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-verified-hooks";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "https://checkout.stripe.test/session/cs_test_harbor_shop_verified_hooks"
    );
    assert_eq!(checkout_client.take_calls().len(), 1);

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("x-stripe-delivery", "evt_test_verified_hook")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(webhook_response.status(), StatusCode::OK);

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].source, "stripe");
    assert_eq!(recorded[0].event, "payment.captured");
    assert_eq!(
        recorded[0].content_type.as_deref(),
        Some("application/x-www-form-urlencoded")
    );
    assert_eq!(recorded[0].payload, webhook_body);
    assert_eq!(
        recorded[0]
            .headers
            .get("x-davenda-verified-webhook-source")
            .map(String::as_str),
        Some("stripe")
    );
    assert_eq!(
        recorded[0]
            .headers
            .get("x-davenda-verified-webhook-event")
            .map(String::as_str),
        Some("payment.captured")
    );
    assert_eq!(
        recorded[0]
            .headers
            .get("x-davenda-route")
            .map(String::as_str),
        Some("commerce.payment-provider-webhook")
    );
}

#[tokio::test]
async fn server_host_accepts_native_stripe_webhooks_and_exposes_raw_json_to_customer_hooks() {
    let app_name = unique_app_name("harbor-shop-runtime-native-stripe-webhook-hooks");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingVerifiedWebhookPlugin {
            calls: calls.clone(),
        })
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-native-stripe-hooks";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let payload = serde_json::json!({
        "id": "evt_test_native_checkout_complete",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": "cs_test_native_checkout_complete",
                "client_reference_id": "PAY-50001",
                "metadata": {
                    "payment_reference": "PAY-50001"
                }
            }
        }
    })
    .to_string();
    let signature = fresh_stripe_webhook_signature(&payload);
    let response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/json")
                .header("stripe-signature", signature)
                .body(Body::from(payload.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK, "{body}");

    let snapshot = store
        .snapshot(&issued.record.session_id, Some(principal_id))
        .unwrap();
    assert_eq!(snapshot.payment.status, "captured");
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.status.as_str()),
        Some("paid")
    );

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    assert_eq!(recorded[0].source, "stripe");
    assert_eq!(recorded[0].event, "checkout.session.completed");
    assert_eq!(
        recorded[0].content_type.as_deref(),
        Some("application/json")
    );
    assert_eq!(recorded[0].payload, payload);
    assert_eq!(
        recorded[0]
            .headers
            .get("x-davenda-verified-webhook-event")
            .map(String::as_str),
        Some("checkout.session.completed")
    );
    assert_eq!(
        recorded[0]
            .headers
            .get("x-davenda-verified-webhook-delivery-id")
            .map(String::as_str),
        Some("evt_test_native_checkout_complete")
    );
}

#[tokio::test]
async fn server_host_rejects_replayed_native_stripe_webhook_deliveries_across_server_reopen() {
    let app_name = unique_app_name("harbor-shop-runtime-native-stripe-webhook-replay");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let calls = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingVerifiedWebhookPlugin {
            calls: calls.clone(),
        })
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-native-stripe-replay";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let payload = serde_json::json!({
        "id": "evt_test_native_checkout_replay",
        "type": "checkout.session.completed",
        "data": {
            "object": {
                "id": "cs_test_native_checkout_replay",
                "client_reference_id": "PAY-50001",
                "metadata": {
                    "payment_reference": "PAY-50001"
                }
            }
        }
    })
    .to_string();
    let signature = fresh_stripe_webhook_signature(&payload);
    let first = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/json")
                .header("stripe-signature", signature.clone())
                .body(Body::from(payload.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first.status();
    let first_body = String::from_utf8(
        to_bytes(first.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(first_status, StatusCode::OK, "{first_body}");

    let reopened = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let second = reopened
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/json")
                .header("stripe-signature", signature)
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = second.status();
    let body = String::from_utf8(
        to_bytes(second.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body.contains("evt_test_native_checkout_replay"), "{body}");

    let snapshot = store
        .snapshot(&issued.record.session_id, Some(principal_id))
        .unwrap();
    assert_eq!(snapshot.payment.status, "captured");
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.status.as_str()),
        Some("paid")
    );

    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.len(), 1, "{recorded:?}");
}

#[tokio::test]
async fn server_host_rejects_replayed_generic_verified_webhooks_across_server_reopen() {
    let app_name = unique_app_name("harbor-shop-runtime-generic-webhook-replay");
    let config = with_payment_webhook_secret(config_with_app_name(&app_name));
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-generic-replay";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "generic")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("generic", "payment.captured", "PAY-50001"),
        )
        .finish();
    let first = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    let first_status = first.status();
    let first_body = String::from_utf8(
        to_bytes(first.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(first_status, StatusCode::OK, "{first_body}");

    let reopened = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let second = reopened
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = second.status();
    let body = String::from_utf8(
        to_bytes(second.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::CONFLICT);

    let snapshot = store
        .snapshot(&issued.record.session_id, Some(principal_id))
        .unwrap();
    assert_eq!(snapshot.payment.status, "captured");
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.status.as_str()),
        Some("paid")
    );
    assert!(
        body.contains("has already been processed") || body.contains("verified webhook delivery"),
        "{body}"
    );
}

#[tokio::test]
async fn server_host_exposes_persisted_commerce_orders_to_verified_webhook_hooks() {
    let app_name = unique_app_name("harbor-shop-runtime-verified-webhook-orders");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let reads = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingWebhookOrderRepositoryPlugin {
            reads: reads.clone(),
        })
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_order_reader",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-verified-webhook-orders";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(checkout_client.take_calls().len(), 1);

    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("provider", "stripe")
                        .append_pair("event", "payment.captured")
                        .append_pair("payment_reference", "PAY-50001")
                        .append_pair(
                            "signature",
                            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
                        )
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(webhook_response.status(), StatusCode::OK);

    let reads = reads.lock().unwrap().clone();
    assert_eq!(reads.len(), 1);
    assert_eq!(reads[0].payment_reference, "PAY-50001");
    assert_eq!(reads[0].checkout_email, "buyer@example.com");
}

#[tokio::test]
async fn server_host_exposes_typed_catalog_repository_access_to_verified_webhook_hooks() {
    let app_name = unique_app_name("harbor-shop-runtime-verified-webhook-catalog");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let mutations = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingWebhookCatalogRepositoryPlugin {
            mutations: mutations.clone(),
        })
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_catalog_repository",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-verified-webhook-catalog";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(checkout_client.take_calls().len(), 1);

    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("provider", "stripe")
                        .append_pair("event", "payment.captured")
                        .append_pair("payment_reference", "PAY-50001")
                        .append_pair(
                            "signature",
                            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
                        )
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(webhook_response.status(), StatusCode::OK);

    let mutations = mutations.lock().unwrap().clone();
    assert_eq!(mutations.len(), 1);
    assert_eq!(mutations[0].product_handle, "gold-membership");
    assert_eq!(mutations[0].product_title, "Gold Membership Plus");
    assert_eq!(mutations[0].collection_handle, "memberships");
    assert_eq!(mutations[0].collection_label, "Premium perks");

    let product_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/gold-membership")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(product_response.status(), StatusCode::OK);
    let product_body = String::from_utf8(
        to_bytes(product_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        product_body.contains("Gold Membership Plus"),
        "{product_body}"
    );
    assert!(
        product_body.contains("Webhook-adjusted premium access for Harbor members."),
        "{product_body}"
    );
    assert!(product_body.contains("£94.00"), "{product_body}");

    let collection_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(collection_response.status(), StatusCode::OK);
    let collection_body = String::from_utf8(
        to_bytes(collection_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        collection_body.contains("Webhook-adjusted membership merchandising copy."),
        "{collection_body}"
    );
    assert!(
        collection_body.contains("Gold Membership Plus"),
        "{collection_body}"
    );
}

#[tokio::test]
async fn server_host_runs_asset_capable_sdk_verified_webhook_hooks_in_live_payment_webhook_flow() {
    let app_name = unique_app_name("harbor-shop-runtime-verified-webhook-assets");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let object_store_server = ObjectStoreTestServer::spawn();
    let object_store_secret = object_store_secret(object_store_server.endpoint());
    config.app.environment = Environment::Development;
    config.storage.deployment = StorageDeployment::SingleNode;
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let writes = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RecordingVerifiedWebhookAssetPlugin {
            writes: writes.clone(),
        })
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook_and_object_store_secret(
        &object_store_secret,
    );
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_verified_hook_assets",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-verified-hook-assets";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "harbor-cap")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("payment_last4", "4242")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "https://checkout.stripe.test/session/cs_test_harbor_shop_verified_hook_assets"
    );
    assert_eq!(checkout_client.take_calls().len(), 1);

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let webhook_status = webhook_response.status();
    let webhook_response_body = String::from_utf8(
        to_bytes(webhook_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(webhook_status, StatusCode::OK, "{webhook_response_body}");

    let writes = writes.lock().unwrap().clone();
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0].storage_class, "public_upload");
    assert!(
        writes[0].logical_path.contains("uploads/customer-hooks/"),
        "{writes:?}"
    );
    assert!(
        writes[0].storage_path.contains("uploads/customer-hooks/"),
        "{writes:?}"
    );
    assert!(writes[0].bytes_written > 0, "{writes:?}");
    assert!(
        writes[0]
            .public_url
            .as_deref()
            .is_some_and(|url| url.contains("uploads/customer-hooks/")),
        "{writes:?}"
    );
}

#[tokio::test]
async fn server_host_persists_linked_customer_managed_assets_across_requests() {
    let app_name = unique_app_name("harbor-shop-runtime-persisted-verified-webhook-assets");
    let object_store_server = ObjectStoreTestServer::spawn();
    let object_store_secret = object_store_secret(object_store_server.endpoint());
    let template_root = checked_in_harbor_shop_root();

    let mut config = checked_in_harbor_shop_config(&app_name);
    config.app.environment = Environment::Development;
    config.storage.deployment = StorageDeployment::SingleNode;
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let writes = Arc::new(Mutex::new(Vec::new()));
    let inspected = Arc::new(Mutex::new(Vec::new()));
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(WritingThenInspectingVerifiedWebhookAssetPlugin {
            writes: writes.clone(),
            inspected: inspected.clone(),
        })
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook_and_object_store_secret(
        &object_store_secret,
    );
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-persisted-verified-hook-assets";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let first_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_response.status(), StatusCode::OK);

    let second_webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.failed")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.failed", "PAY-50001"),
        )
        .finish();
    let second_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(second_webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_response.status(), StatusCode::OK);

    let inspected = inspected.lock().unwrap().clone();
    assert_eq!(inspected.len(), 1, "{inspected:?}");
    assert_eq!(
        inspected[0].logical_path,
        format!("uploads/customer-hooks/{app_name}/payment.captured.json")
    );
    assert_eq!(inspected[0].storage_class, "public_upload");
    assert!(
        inspected[0]
            .public_url
            .as_deref()
            .is_some_and(|url| url.contains("uploads/customer-hooks/")),
        "{inspected:?}"
    );
}

#[tokio::test]
async fn server_host_rejects_payment_webhook_mutation_when_sdk_hook_rejects_it() {
    let app_name = unique_app_name("harbor-shop-runtime-verified-webhook-rejection");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RejectVerifiedWebhookPlugin)
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-verified-rejection";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "harbor-cap",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(webhook_response.status(), StatusCode::CONFLICT);

    let snapshot = store
        .snapshot(&issued.record.session_id, Some(principal_id))
        .unwrap();
    assert_eq!(snapshot.payment.status, "provider_pending");
    assert_eq!(
        snapshot
            .latest_order
            .as_ref()
            .map(|order| order.status.as_str()),
        Some("pending_payment")
    );
}

#[tokio::test]
async fn server_host_executes_checked_in_harbor_shop_stripe_checkout_handoff_and_webhook() {
    let app_name = unique_app_name("harbor-shop-runtime-stripe-handoff");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_handoff",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-stripe-handoff";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let product_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/gold-membership")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let product_body = String::from_utf8(
        to_bytes(product_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(product_body.contains("Gold Membership"), "{product_body}");
    assert!(product_body.contains("Add to cart"), "{product_body}");

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&add_response, "location"), "/cart");

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let cart_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(cart_body.contains("Gold Membership"), "{cart_body}");
    assert!(cart_body.contains("£89.00"), "{cart_body}");

    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        checkout_body.contains("Stripe hosted checkout"),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains(
            "This checkout reserves the order in Davenda, then redirects the customer to Stripe Checkout for payment collection. Davenda still waits for the signed Stripe webhook before treating the order as paid."
        ),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains("Continue to Stripe"),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains("Ready for payment"),
        "{checkout_body}"
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "https://checkout.stripe.test/session/cs_test_harbor_shop_handoff"
    );
    let checkout_calls = checkout_client.take_calls();
    assert_eq!(checkout_calls.len(), 1);
    assert_eq!(checkout_calls[0].api_key, STRIPE_SECRET_KEY);
    assert_eq!(checkout_calls[0].idempotency_key, "davenda-order-ORD-10042");
    assert!(
        checkout_calls[0]
            .request_body
            .contains("success_url=http%3A%2F%2Fwww.example.com%2Fcheckout%2Fconfirmation%3Fcheckout_session_id%3D%257BCHECKOUT_SESSION_ID%257D"),
        "{:?}",
        checkout_calls[0]
    );
    assert!(
        checkout_calls[0]
            .request_body
            .contains("cancel_url=http%3A%2F%2Fwww.example.com%2Fcheckout%2Fconfirmation%3Fprovider_result%3Dcancel%26payment_reference%3DPAY-50001"),
        "{:?}",
        checkout_calls[0]
    );
    let pending_confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation?checkout_session_id=cs_test_harbor_shop_handoff")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let pending_confirmation_body = String::from_utf8(
        to_bytes(pending_confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        pending_confirmation_body.contains("Reference <strong>ORD-10042</strong>"),
        "{pending_confirmation_body}"
    );
    assert!(
        pending_confirmation_body.contains("Status <strong>Pending Payment</strong>"),
        "{pending_confirmation_body}"
    );
    assert!(
        pending_confirmation_body.contains(
            "Stripe Checkout has not confirmed this payment yet. The order will move forward after the hosted Stripe session finishes and the signed Stripe webhook arrives."
        ),
        "{pending_confirmation_body}"
    );
    assert!(
        pending_confirmation_body.contains("Stripe hosted checkout"),
        "{pending_confirmation_body}"
    );

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let webhook_status = webhook_response.status();
    let webhook_response_body = String::from_utf8(
        to_bytes(webhook_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(webhook_status, StatusCode::OK, "{webhook_response_body}");
    assert!(
        webhook_response_body.contains("\"status\":\"accepted\""),
        "{webhook_response_body}"
    );

    let paid_confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let paid_confirmation_body = String::from_utf8(
        to_bytes(paid_confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        paid_confirmation_body.contains("Status <strong>Paid</strong>"),
        "{paid_confirmation_body}"
    );
    assert!(
        !paid_confirmation_body.contains("Stripe still needs to confirm payment."),
        "{paid_confirmation_body}"
    );

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(account_body.contains("Gold Membership"), "{account_body}");
    assert!(account_body.contains("Active"), "{account_body}");
    assert!(
        account_body.contains("Activated from order ORD-10042."),
        "{account_body}"
    );

    let memberships_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let memberships_body = String::from_utf8(
        to_bytes(memberships_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        memberships_body.contains("Gold Membership"),
        "{memberships_body}"
    );
    assert!(memberships_body.contains("Active"), "{memberships_body}");
    assert!(
        memberships_body.contains("Activated from order ORD-10042."),
        "{memberships_body}"
    );
}

#[tokio::test]
async fn server_host_reconciles_paid_stripe_checkout_session_on_provider_return() {
    let app_name = unique_app_name("harbor-shop-runtime-stripe-session-return");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url_and_status(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_return_paid",
        Some("complete"),
        Some("paid"),
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-stripe-session-return";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "https://checkout.stripe.test/session/cs_test_harbor_shop_return_paid"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation?checkout_session_id=cs_test_harbor_shop_return_paid")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("Status <strong>Paid</strong>"),
        "{confirmation_body}"
    );
    assert!(
        !confirmation_body.contains("Stripe Checkout has not confirmed this payment yet."),
        "{confirmation_body}"
    );

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(account_body.contains("Gold Membership"), "{account_body}");
    assert!(account_body.contains("Active"), "{account_body}");
}

#[tokio::test]
async fn server_host_completes_checked_in_harbor_shop_local_checkout_stub_with_placeholder_stripe_key()
 {
    let app_name = unique_app_name("harbor-shop-runtime-local-checkout-stub");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.app.environment = davenda_config::Environment::Development;
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_placeholder_stripe();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("dev-customer")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "http://localhost:8080/checkout/confirmation?provider_result=return&payment_reference=PAY-50001"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation?provider_result=return&payment_reference=PAY-50001")
                .header("host", "localhost:8080")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("Status <strong>Paid</strong>"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Stripe hosted checkout"),
        "{confirmation_body}"
    );
}

#[tokio::test]
async fn server_host_executes_checked_in_harbor_shop_stripe_checkout_reconciliation_requires_signed_webhook()
 {
    let app_name = unique_app_name("harbor-shop-runtime-stripe-reconciliation");
    let mut config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let checkout_client = Arc::new(StaticHostedCheckoutClient::with_url(
        "https://checkout.stripe.test/session/cs_test_harbor_shop_reconciliation",
    ));
    let server = plan
        .server_host_with_checkout_client(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
            checkout_client.clone(),
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "member-live-stripe-reconciliation";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        checkout_body.contains("Stripe hosted checkout"),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains(
            "This checkout reserves the order in Davenda, then redirects the customer to Stripe Checkout for payment collection. Davenda still waits for the signed Stripe webhook before treating the order as paid."
        ),
        "{checkout_body}"
    );
    assert!(
        checkout_body.contains("Continue to Stripe"),
        "{checkout_body}"
    );

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("payment_last4", "4242")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "https://checkout.stripe.test/session/cs_test_harbor_shop_reconciliation"
    );
    let checkout_calls = checkout_client.take_calls();
    assert_eq!(checkout_calls.len(), 1);
    assert_eq!(checkout_calls[0].api_key, STRIPE_SECRET_KEY);

    let pending_confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri(
                    "/checkout/confirmation?checkout_session_id=cs_test_harbor_shop_reconciliation",
                )
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let pending_confirmation_body = String::from_utf8(
        to_bytes(pending_confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        pending_confirmation_body.contains("Reference <strong>ORD-10042</strong>"),
        "{pending_confirmation_body}"
    );
    assert!(
        pending_confirmation_body.contains("Status <strong>Pending Payment</strong>"),
        "{pending_confirmation_body}"
    );
    assert!(
        pending_confirmation_body.contains(
            "Stripe Checkout has not confirmed this payment yet. The order will move forward after the hosted Stripe session finishes and the signed Stripe webhook arrives."
        ),
        "{pending_confirmation_body}"
    );

    let invalid_webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair("signature", "not-valid")
        .finish();
    let invalid_webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(invalid_webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let invalid_webhook_status = invalid_webhook_response.status();
    let invalid_webhook_body = String::from_utf8(
        to_bytes(invalid_webhook_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(invalid_webhook_status, StatusCode::BAD_REQUEST);
    assert!(
        invalid_webhook_body.contains("payment webhook verification failed"),
        "{invalid_webhook_body}"
    );

    let still_pending_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let still_pending_body = String::from_utf8(
        to_bytes(still_pending_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        still_pending_body.contains("Status <strong>Pending Payment</strong>"),
        "{still_pending_body}"
    );
    assert!(
        still_pending_body.contains("Status <strong>Pending Payment</strong>"),
        "{still_pending_body}"
    );
    assert!(
        still_pending_body.contains(
            "Stripe Checkout has not confirmed this payment yet. The order will move forward after the hosted Stripe session finishes and the signed Stripe webhook arrives."
        ),
        "{still_pending_body}"
    );

    let webhook_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("provider", "stripe")
        .append_pair("event", "payment.captured")
        .append_pair("payment_reference", "PAY-50001")
        .append_pair(
            "signature",
            &payment_webhook_signature("stripe", "payment.captured", "PAY-50001"),
        )
        .finish();
    let webhook_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/webhooks/commerce/payment-provider")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(webhook_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let webhook_status = webhook_response.status();
    let webhook_response_body = String::from_utf8(
        to_bytes(webhook_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(webhook_status, StatusCode::OK, "{webhook_response_body}");
    assert!(
        webhook_response_body.contains("\"status\":\"accepted\""),
        "{webhook_response_body}"
    );

    let paid_confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let paid_confirmation_body = String::from_utf8(
        to_bytes(paid_confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        paid_confirmation_body.contains("Status <strong>Paid</strong>"),
        "{paid_confirmation_body}"
    );
    assert!(
        !paid_confirmation_body.contains("Stripe still needs to confirm payment."),
        "{paid_confirmation_body}"
    );
    assert!(
        paid_confirmation_body.contains("Card ending 4242, reference PAY-50001"),
        "{paid_confirmation_body}"
    );

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(account_body.contains("Gold Membership"), "{account_body}");
    assert!(account_body.contains("Active"), "{account_body}");
    assert!(
        account_body.contains("Activated from order ORD-10042."),
        "{account_body}"
    );

    let order_history_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let order_history_body = String::from_utf8(
        to_bytes(order_history_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        order_history_body.contains("ORD-10042"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Card ending 4242, reference PAY-50001"),
        "{order_history_body}"
    );
    assert!(
        order_history_body.contains("Gold Membership"),
        "{order_history_body}"
    );
    assert!(order_history_body.contains("Paid"), "{order_history_body}");
}

#[tokio::test]
async fn server_host_executes_checked_in_harbor_shop_french_customer_journey() {
    let app_name = unique_app_name("harbor-shop-runtime-french-customer-journey");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let mut config = config;
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_route(RouteDefinition::new("home", HttpMethod::Get, "/").unwrap())
        .with_handler(HandlerDefinition::page("home", "pages/home").unwrap())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(SessionIssueRequest::new(), now)
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let collection_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/fr-FR/shop/collections/memberships")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collection_body = String::from_utf8(
        to_bytes(collection_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        collection_body.contains("lang=\"fr-FR\""),
        "{collection_body}"
    );
    assert!(
        collection_body.contains("/fr-FR/shop/products/gold-membership"),
        "{collection_body}"
    );
    assert!(
        collection_body.contains("Gold Membership"),
        "{collection_body}"
    );

    let product_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/fr-FR/shop/products/gold-membership")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let product_body = String::from_utf8(
        to_bytes(product_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(product_body.contains("lang=\"fr-FR\""), "{product_body}");
    assert!(
        product_body.contains("value=\"gold-membership\""),
        "{product_body}"
    );
    assert!(product_body.contains("Add to cart"), "{product_body}");

    let cart_bootstrap = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let add_token = response_header(
        &cart_bootstrap,
        "x-davenda-storefront-csrf-commerce-add-to-cart",
    );
    let add_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/cart/items")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", add_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("product_slug", "gold-membership")
                        .append_pair("quantity", "1")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(add_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&add_response, "location"), "/cart");

    let cart_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/cart")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_start_token = response_header(
        &cart_response,
        "x-davenda-storefront-csrf-commerce-checkout-start",
    );
    let cart_body = String::from_utf8(
        to_bytes(cart_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(cart_body.contains("Gold Membership"), "{cart_body}");
    assert!(cart_body.contains("£89.00"), "{cart_body}");

    let checkout_start = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/start")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_start_token)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(checkout_start.status(), StatusCode::SEE_OTHER);
    assert_eq!(response_header(&checkout_start, "location"), "/checkout");

    let checkout_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let checkout_complete_token = response_header(
        &checkout_response,
        "x-davenda-storefront-csrf-commerce-checkout-complete",
    );
    let checkout_body = String::from_utf8(
        to_bytes(checkout_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(checkout_body.contains("Gold Membership"), "{checkout_body}");
    assert!(checkout_body.contains("PAY-50001"), "{checkout_body}");

    let complete_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/checkout/complete")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("x-csrf-token", checkout_complete_token)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("checkout_email", "buyer@example.com")
                        .append_pair("payment_method", "card")
                        .append_pair("payment_last4", "4242")
                        .append_pair("checkout_intent", "PAY-50001")
                        .append_pair("terms_accepted", "yes")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(complete_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&complete_response, "location"),
        "/checkout/confirmation"
    );

    let confirmation_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/checkout/confirmation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let confirmation_body = String::from_utf8(
        to_bytes(confirmation_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        confirmation_body.contains("ORD-10042"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Pending Payment"),
        "{confirmation_body}"
    );
    assert!(
        confirmation_body.contains("Card ending 4242"),
        "{confirmation_body}"
    );

    let account_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/account")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let account_body = String::from_utf8(
        to_bytes(account_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        account_body.contains("Membership access moves into this account area"),
        "{account_body}"
    );
    assert!(account_body.contains("Pending Payment"), "{account_body}");
    assert!(account_body.contains("buyer@example.com"), "{account_body}");
}

#[tokio::test]
async fn server_host_renders_honest_checked_in_harbor_shop_events_surfaces() {
    let template_root = checked_in_harbor_shop_root();
    let mut config = config_with_app_name("harbor-shop");
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(EventsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let events_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/events")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let events_status = events_response.status();
    let events_body = String::from_utf8(
        to_bytes(events_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(events_status, StatusCode::OK, "{events_body}");
    assert!(
        events_body.contains("Events are enabled, but the sample catalog is still being wired."),
        "{events_body}"
    );
    assert!(events_body.contains("events.list"), "{events_body}");
    assert!(
        events_body.contains("Browse event-linked offers"),
        "{events_body}"
    );
    assert!(events_body.contains("Review memberships"), "{events_body}");
    assert!(events_body.contains("lang=\"en-GB\""), "{events_body}");
    assert!(!events_body.contains("runtime.page.shell"), "{events_body}");

    let event_detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/fr-FR/events/spring-tasting")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let event_detail_status = event_detail_response.status();
    let event_detail_body = String::from_utf8(
        to_bytes(event_detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(event_detail_status, StatusCode::OK, "{event_detail_body}");
    assert!(
        event_detail_body.contains("spring-tasting"),
        "{event_detail_body}"
    );
    assert!(
        event_detail_body
            .contains("Event records are not published in the checked-in Harbor Shop sample yet"),
        "{event_detail_body}"
    );
    assert!(
        event_detail_body.contains("Review memberships"),
        "{event_detail_body}"
    );
    assert!(
        event_detail_body.contains("Open account"),
        "{event_detail_body}"
    );
    assert!(
        event_detail_body.contains("lang=\"fr-FR\""),
        "{event_detail_body}"
    );
    assert!(
        !event_detail_body.contains("runtime.page.shell"),
        "{event_detail_body}"
    );
}

#[tokio::test]
async fn server_host_emits_hreflang_links_for_localized_page_routes() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("events.list", HttpMethod::Get, "/events")
                .unwrap()
                .localized(),
        )
        .with_handler(HandlerDefinition::page("events.list", "events/list").unwrap())
        .with_template(page_template(customer_namespace, "events/list"))
        .build()
        .unwrap();

    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/fr-FR/events")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let _headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    if status != StatusCode::OK {
        panic!("api response failed: status={status}, body={body}");
    }
    assert!(body.contains("hreflang=\"fr-FR\""));
    assert!(body.contains("https://www.example.com/fr-FR/events"));
    assert!(body.contains("hreflang=\"en-GB\""));
    assert!(body.contains("https://www.example.com/en-GB/events"));
}

#[test]
fn runtime_plan_exposes_declared_outbound_http_endpoints() {
    let config = config_with_outbound_http();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    assert_eq!(
        plan.approved_outbound_http_endpoints()
            .get("crm")
            .map(|endpoint| endpoint.as_str()),
        Some("https://crm.example.com/api")
    );
}

#[tokio::test]
async fn server_host_renders_fragment_templates_as_html() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("cms.preview", HttpMethod::Get, "/fragments/preview")
                .unwrap()
                .with_area(RouteArea::Fragment),
        )
        .with_handler(
            HandlerDefinition::fragment("cms.preview", "cms/preview", "preview-pane").unwrap(),
        )
        .with_template(fragment_template(customer_namespace, "cms/preview"))
        .build()
        .unwrap();

    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/fragments/preview")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        headers.get("content-type").unwrap(),
        "text/html; charset=utf-8"
    );
    assert!(body.contains("id=\"preview-pane\""));
    assert!(body.contains("/fragments/preview"));
}

#[tokio::test]
async fn server_host_executes_page_extensions_during_live_requests() {
    let app_name = "showcase-events-page-wasm";
    let extension_dir = unique_temp_extension_dir("page-wasm");
    fs::create_dir_all(&extension_dir).unwrap();
    let config = config_with_app_name_and_extension_directory(&extension_dir, app_name);
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let page_slots = StaticManifestModule::new(
        ModuleManifest::new("account.runtime.slot").with_extension_slots(vec![
            ExtensionSlotDescriptor::new(
                ExtensionSlotKind::Page,
                "/account",
                "Allows account page extensions to participate in the live request path",
            ),
        ]),
    );
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(page_slots)
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .with_template(page_template(customer_namespace, "account/dashboard"))
        .with_installed_extension(installed_page_extension_for_app_with_artifact(
            &extension_dir,
            "/account",
            app_name,
        ))
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-3")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/account")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(
        headers.get("x-davenda-wasm-request-handler").unwrap(),
        "account-dashboard"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-request-outcome").unwrap(),
        "Page"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-metadata-title").unwrap(),
        "Account Runtime Extension"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-cache-visibility").unwrap(),
        "public"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-cache-tags").unwrap(),
        "account-runtime"
    );
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "public,max-age=60,stale-while-revalidate=30,vary-by-locale"
    );
    assert!(
        headers
            .get("surrogate-key")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("account-runtime")
    );
    assert!(body.contains("Account runtime extension"));
    assert!(body.contains("data-route=\"account.dashboard\""));
    assert!(body.contains("Account Runtime Extension"));

    fs::remove_dir_all(&extension_dir).unwrap();
}

#[tokio::test]
async fn server_host_applies_typed_cache_policy_to_public_page_responses() {
    let app_name = "showcase-events-public-page-wasm";
    let extension_dir = unique_temp_extension_dir("public-page-wasm");
    fs::create_dir_all(&extension_dir).unwrap();
    let config = config_with_app_name_and_extension_directory(&extension_dir, app_name);
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let page_slots = StaticManifestModule::new(
        ModuleManifest::new("events.runtime.slot").with_extension_slots(vec![
            ExtensionSlotDescriptor::new(
                ExtensionSlotKind::Page,
                "/events",
                "Allows public page extensions to participate in the live request path",
            ),
        ]),
    );
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(page_slots)
        .with_route(RouteDefinition::new("events.public", HttpMethod::Get, "/events").unwrap())
        .with_handler(HandlerDefinition::page("events.public", "events/list").unwrap())
        .with_template(page_template(customer_namespace, "events/list"))
        .with_installed_extension(installed_page_extension_for_app_with_artifact(
            &extension_dir,
            "/events",
            app_name,
        ))
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(&resolver, cookie_secret, csrf_secret)
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/events")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(
        headers.get("cache-control").unwrap(),
        "public,max-age=60,stale-while-revalidate=30,vary-by-locale"
    );
    assert!(
        headers
            .get("surrogate-key")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("account-runtime")
    );
    assert!(body.contains("Account runtime extension"));

    fs::remove_dir_all(&extension_dir).unwrap();
}

#[tokio::test]
async fn server_host_executes_render_hooks_during_html_render() {
    let extension_dir = unique_temp_extension_dir("render-hook-wasm");
    fs::create_dir_all(&extension_dir).unwrap();
    let config = config_with_extension_directory(&extension_dir);
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_template(page_template(customer_namespace, "cms/page"))
        .with_installed_extension(installed_render_hook_extension_with_artifact(
            &extension_dir,
        ))
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/en-GB/pages/home")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        headers.get("x-davenda-wasm-render-hook-count").unwrap(),
        "1"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-render-hook-handlers").unwrap(),
        "loyalty-badge"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-metadata-description").unwrap(),
        "render hook output for loyalty badges"
    );
    assert!(body.contains("rel=\"canonical\""));
    assert!(body.contains("Loyalty badge"));

    fs::remove_dir_all(&extension_dir).unwrap();
}

#[tokio::test]
async fn server_host_executes_admin_widget_extensions_during_live_requests() {
    let extension_dir = unique_temp_extension_dir("admin-widget-wasm");
    fs::create_dir_all(&extension_dir).unwrap();
    let config = config_with_extension_directory(&extension_dir);
    let customer_namespace = TemplateNamespace::new("customer-app").unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_template(page_template(customer_namespace, "admin/dashboard"))
        .with_installed_extension(installed_admin_widget_extension_with_artifact(
            &extension_dir,
        ))
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let authorizer = Arc::new(PermissiveLiveRouteCapabilityAuthorizer);
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        authorizer,
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/admin")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get("x-davenda-wasm-admin-widget-count").unwrap(),
        "1"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-admin-widget-handlers").unwrap(),
        "waitlist-summary"
    );
    assert!(body.contains("Waitlist widget"));

    fs::remove_dir_all(&extension_dir).unwrap();
}

#[tokio::test]
async fn server_host_renders_checked_in_harbor_shop_admin_surfaces() {
    let template_root = checked_in_harbor_shop_root();
    let mut config = config_with_app_name(&unique_app_name("harbor-shop-runtime-admin-surfaces"));
    config.auth.package = "harbor-auth".to_string();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();

    for (route, expected) in [
        ("/admin", "Harbor Shop Admin"),
        ("/admin/audit", "Audit Log"),
        ("/admin/orders", "Orders"),
        ("/admin/catalog/products", "Catalog Administration"),
        ("/admin/pages", "Pages"),
        ("/admin/navigation", "Navigation"),
        ("/admin/redirects", "Redirects"),
    ] {
        let response = server
            .respond(
                Request::builder()
                    .method("GET")
                    .uri(route)
                    .header("host", "www.example.com")
                    .header("x-forwarded-proto", "https")
                    .header("cookie", format!("davenda_session={}", issued.cookie_value))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();

        assert_eq!(status, StatusCode::OK, "{route}");
        assert!(body.contains(expected), "{route}: {body}");

        match route {
            "/admin" => {
                assert!(body.contains("Launch sign-off"), "{route}: {body}");
                assert!(body.contains("Manage live catalog copy"), "{route}: {body}");
                assert!(body.contains("Order support baseline"), "{route}: {body}");
                assert!(body.contains("Cutover content checks"), "{route}: {body}");
            }
            "/admin/audit" => {
                assert!(
                    body.contains("No privileged admin actions captured yet"),
                    "{route}: {body}"
                );
                assert!(
                    body.contains("Backend <code>local-sqlite</code>"),
                    "{route}: {body}"
                );
                assert!(body.contains("Capture a support action"), "{route}: {body}");
                assert!(body.contains("Open pages"), "{route}: {body}");
            }
            "/admin/orders" => {
                assert!(body.contains("Support first"), "{route}: {body}");
                assert!(
                    body.contains("Pending Payment is not failure"),
                    "{route}: {body}"
                );
                assert!(body.contains("Refund boundary"), "{route}: {body}");
            }
            "/admin/catalog/products" => {
                assert!(body.contains("Save product"), "{route}: {body}");
                assert!(body.contains("Live browse truth"), "{route}: {body}");
                assert!(body.contains("Visible in storefront"), "{route}: {body}");
                assert!(body.contains("Save collection"), "{route}: {body}");
            }
            "/admin/pages" => {
                assert!(body.contains("Draft workflow"), "{route}: {body}");
                assert!(body.contains("Save draft"), "{route}: {body}");
                assert!(body.contains("Draft preview"), "{route}: {body}");
                assert!(body.contains("Create page"), "{route}: {body}");
            }
            "/admin/navigation" => {
                assert!(body.contains("Primary navigation"), "{route}: {body}");
                assert!(body.contains("Save navigation"), "{route}: {body}");
            }
            "/admin/redirects" => {
                assert!(body.contains("Redirect rules"), "{route}: {body}");
                assert!(body.contains("Save redirects"), "{route}: {body}");
            }
            _ => {}
        }
    }
}

#[tokio::test]
async fn server_host_executes_checked_in_harbor_shop_cms_page_draft_and_publish_workflow() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-draft-publish");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-membership-guide")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let draft_token = response_header(&admin_response, "x-davenda-cms-csrf-cms-pages-save-draft");
    let admin_body = String::from_utf8(
        to_bytes(admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    let draft_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &draft_token)
        .append_pair("page_id", "page-membership-guide")
        .append_pair("page_title", "Harbor Membership Access")
        .append_pair("page_slug", "harbor-membership-access")
        .append_pair(
            "page_summary",
            "Explains what customers unlock after checkout and how activation appears in account.",
        )
        .append_pair(
            "page_body_html",
            "<p>Customers can review pending activation immediately after checkout.</p><p>Publishing from Harbor Shop admin makes this page live for the storefront.</p>",
        )
        .finish();
    let draft_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/draft")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(draft_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let draft_status = draft_response.status();
    let draft_headers = draft_response.headers().clone();
    let draft_body_text = String::from_utf8(
        to_bytes(draft_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(draft_status, StatusCode::SEE_OTHER, "{draft_body_text}");
    assert_eq!(
        draft_headers.get("location").unwrap().to_str().unwrap(),
        "/admin/pages?page=page-membership-guide"
    );
    let draft_flash =
        cookie_pair_from_headers(&draft_headers, "davenda_flash").expect("draft save flash");

    let draft_admin_cookie = format!("{session_cookie}; {draft_flash}");
    let draft_admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-membership-guide")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &draft_admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let publish_token = response_header(
        &draft_admin_response,
        "x-davenda-cms-csrf-cms-pages-publish",
    );
    let draft_admin_body = String::from_utf8(
        to_bytes(draft_admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        draft_admin_body.contains("Draft saved. Preview and publish when ready."),
        "{draft_admin_body}"
    );
    assert!(
        draft_admin_body.contains("Harbor Membership Access"),
        "{draft_admin_body}"
    );
    assert!(
        draft_admin_body.contains("harbor-membership-access"),
        "{draft_admin_body}"
    );
    assert!(
        draft_admin_body
            .contains("Customers can review pending activation immediately after checkout."),
        "{draft_admin_body}"
    );

    let publish_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &publish_token)
        .append_pair("page_id", "page-membership-guide")
        .finish();
    let publish_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/publish")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(publish_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&publish_response, "location"),
        "/admin/pages?page=page-membership-guide"
    );

    let live_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/pages/harbor-membership-access")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(live_page.status(), StatusCode::OK);
    let live_page_body = String::from_utf8(
        to_bytes(live_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        live_page_body.contains("Harbor Membership Access"),
        "{live_page_body}"
    );
    assert!(
        live_page_body
            .contains("Customers can review pending activation immediately after checkout."),
        "{live_page_body}"
    );
    assert!(
        live_page_body
            .contains("Publishing from Harbor Shop admin makes this page live for the storefront."),
        "{live_page_body}"
    );

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Save draft"), "{audit_body}");
    assert!(audit_body.contains("Publish page"), "{audit_body}");
    assert!(
        audit_body.contains("page:page-membership-guide"),
        "{audit_body}"
    );
    assert!(audit_body.contains("cms.page.publish"), "{audit_body}");
}

#[tokio::test]
async fn server_host_allows_linked_cms_hooks_to_rewrite_the_draft_before_publish() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-rewrite-publish");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RewriteCmsPublishPlugin)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-cms-rewriter")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-membership-guide")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let publish_token = response_header(&admin_response, "x-davenda-cms-csrf-cms-pages-publish");

    let publish_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &publish_token)
        .append_pair("page_id", "page-membership-guide")
        .append_pair("page_title", "Harbor Membership Access")
        .append_pair("page_slug", "harbor-membership-access")
        .append_pair(
            "page_summary",
            "Explains what customers unlock after checkout and how activation appears in account.",
        )
        .append_pair(
            "page_body_html",
            "<p>Customers can review pending activation immediately after checkout.</p>",
        )
        .finish();
    let publish_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/publish")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(publish_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish_response.status(), StatusCode::SEE_OTHER);

    let live_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/pages/harbor-membership-access")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let live_body = String::from_utf8(
        to_bytes(live_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        live_body.contains("Harbor Membership Access (Linked review)"),
        "{live_body}"
    );
    assert!(
        live_body.contains("Linked customer review updated this page before publish."),
        "{live_body}"
    );
}

#[tokio::test]
async fn server_host_runs_sdk_cms_publish_hooks_before_live_publish() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-hooks");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .register_customer_plugin(RejectCmsPublishPlugin)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-cms-hooks")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let new_page_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?new=1")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let draft_token = response_header(
        &new_page_response,
        "x-davenda-cms-csrf-cms-pages-save-draft",
    );
    let draft_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &draft_token)
        .append_pair("page_title", "SDK Hook Review Page")
        .append_pair("page_slug", "sdk-hook-review-page")
        .append_pair(
            "page_summary",
            "A unique draft used to prove linked customer CMS publish hooks can block live publication.",
        )
        .append_pair(
            "page_body_html",
            "<p>This page should remain draft-only because the linked customer publish hook rejects it.</p>",
        )
        .finish();
    let draft_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/draft")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(draft_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(draft_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&draft_response, "location"),
        "/admin/pages?page=page-sdk-hook-review-page"
    );

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-sdk-hook-review-page")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let publish_token = response_header(&admin_response, "x-davenda-cms-csrf-cms-pages-publish");

    let publish_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &publish_token)
        .append_pair("page_id", "page-sdk-hook-review-page")
        .finish();
    let publish_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/publish")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(publish_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&publish_response, "location"),
        "/admin/pages?page=page-sdk-hook-review-page"
    );
    let flash_cookie = cookie_pair_from_response(&publish_response, "davenda_flash")
        .expect("publish rejection flash");

    let retry_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-sdk-hook-review-page")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", format!("{session_cookie}; {flash_cookie}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let retry_body = String::from_utf8(
        to_bytes(retry_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        retry_body.contains(
            "Linked customer policy blocked this page from publishing until editorial review is complete."
        ),
        "{retry_body}"
    );

    let live_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/pages/sdk-hook-review-page")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(live_page.status(), StatusCode::OK);
    let live_page_body = String::from_utf8(
        to_bytes(live_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        live_page_body.contains("Page unavailable"),
        "{live_page_body}"
    );
    assert!(
        live_page_body.contains("not published yet"),
        "{live_page_body}"
    );
}

#[tokio::test]
async fn server_host_creates_and_publishes_new_checked_in_harbor_shop_cms_page() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-new-page");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let new_page_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?new=1")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let draft_token = response_header(
        &new_page_response,
        "x-davenda-cms-csrf-cms-pages-save-draft",
    );
    let new_page_body = String::from_utf8(
        to_bytes(new_page_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(new_page_body.contains("New page draft"), "{new_page_body}");
    assert!(new_page_body.contains("Create draft"), "{new_page_body}");
    assert!(
        new_page_body.contains("Save the draft first to generate the stable preview route"),
        "{new_page_body}"
    );

    let draft_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &draft_token)
        .append_pair("page_title", "Shipping & Returns")
        .append_pair("page_slug", "shipping-returns")
        .append_pair(
            "page_summary",
            "Explains delivery windows, returns, and what customers should expect after checkout.",
        )
        .append_pair(
            "page_body_html",
            "<p>Orders dispatch within two working days.</p><p>Returns can be started from support after the parcel arrives.</p>",
        )
        .finish();
    let draft_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/draft")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(draft_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(draft_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&draft_response, "location"),
        "/admin/pages?page=page-shipping-returns"
    );

    let saved_admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-shipping-returns")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let publish_token = response_header(
        &saved_admin_response,
        "x-davenda-cms-csrf-cms-pages-publish",
    );
    let saved_admin_body = String::from_utf8(
        to_bytes(saved_admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        saved_admin_body.contains("Shipping &amp; Returns"),
        "{saved_admin_body}"
    );
    assert!(
        saved_admin_body.contains("page-shipping-returns"),
        "{saved_admin_body}"
    );
    assert!(
        saved_admin_body.contains("Publish live page"),
        "{saved_admin_body}"
    );
    assert!(
        saved_admin_body.contains("/admin/pages/preview?page=page-shipping-returns"),
        "{saved_admin_body}"
    );

    let publish_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &publish_token)
        .append_pair("page_id", "page-shipping-returns")
        .finish();
    let publish_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/publish")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(publish_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&publish_response, "location"),
        "/admin/pages?page=page-shipping-returns"
    );

    let live_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/pages/shipping-returns")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(live_page.status(), StatusCode::OK);
    let live_page_body = String::from_utf8(
        to_bytes(live_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        live_page_body.contains("Shipping &amp; Returns"),
        "{live_page_body}"
    );
    assert!(
        live_page_body.contains("Orders dispatch within two working days."),
        "{live_page_body}"
    );
    assert!(
        live_page_body.contains("Returns can be started from support after the parcel arrives."),
        "{live_page_body}"
    );
}

#[tokio::test]
async fn server_host_allows_linked_cms_hooks_to_update_navigation_and_redirects_before_publish() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-workspace-hook");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .register_customer_plugin(RewriteCmsWorkspacePublishPlugin)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let new_page_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?new=1")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let draft_token = response_header(
        &new_page_response,
        "x-davenda-cms-csrf-cms-pages-save-draft",
    );

    let draft_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &draft_token)
        .append_pair("page_title", "Shipping & Returns")
        .append_pair("page_slug", "shipping-returns")
        .append_pair(
            "page_summary",
            "Explains delivery windows, returns, and what customers should expect after checkout.",
        )
        .append_pair(
            "page_body_html",
            "<p>Orders dispatch within two working days.</p><p>Returns can be started from support after the parcel arrives.</p>",
        )
        .finish();
    let draft_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/draft")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(draft_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(draft_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&draft_response, "location"),
        "/admin/pages?page=page-shipping-returns"
    );

    let saved_admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-shipping-returns")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let publish_token = response_header(
        &saved_admin_response,
        "x-davenda-cms-csrf-cms-pages-publish",
    );

    let publish_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &publish_token)
        .append_pair("page_id", "page-shipping-returns")
        .finish();
    let publish_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/publish")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(publish_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(publish_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&publish_response, "location"),
        "/admin/pages?page=page-shipping-returns"
    );

    let home_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let home_body = String::from_utf8(
        to_bytes(home_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(home_body.contains(">Shipping<"), "{home_body}");
    assert!(home_body.contains("/pages/shipping-returns"), "{home_body}");

    let redirect_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/legacy/shipping")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(redirect_response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response_header(&redirect_response, "location"),
        "/pages/shipping-returns"
    );
}

#[tokio::test]
async fn server_host_renders_checked_in_harbor_shop_cms_preview_for_saved_draft() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-preview");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages?page=page-membership-guide")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let draft_token = response_header(&admin_response, "x-davenda-cms-csrf-cms-pages-save-draft");

    let draft_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &draft_token)
        .append_pair("page_id", "page-membership-guide")
        .append_pair("page_title", "Harbor Membership Preview")
        .append_pair("page_slug", "harbor-membership-preview")
        .append_pair(
            "page_summary",
            "Preview the unpublished membership guide before sending customers to it.",
        )
        .append_pair(
            "page_body_html",
            "<p>This unpublished draft should render through the CMS preview endpoint.</p>",
        )
        .finish();
    let draft_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/pages/draft")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(draft_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(draft_response.status(), StatusCode::SEE_OTHER);

    let preview_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/pages/preview?page=page-membership-guide")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview_body = String::from_utf8(
        to_bytes(preview_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        preview_body.contains("id=\"preview-pane\""),
        "{preview_body}"
    );
    assert!(
        preview_body.contains("Harbor Membership Preview"),
        "{preview_body}"
    );
    assert!(
        preview_body
            .contains("This unpublished draft should render through the CMS preview endpoint."),
        "{preview_body}"
    );
}

#[tokio::test]
async fn server_host_updates_checked_in_harbor_shop_navigation_from_cms_admin() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-navigation");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/navigation")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let save_token = response_header(&admin_response, "x-davenda-cms-csrf-cms-navigation-save");

    let save_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &save_token)
        .append_pair("nav_label_0", "Home")
        .append_pair("nav_href_0", "/")
        .append_pair("nav_label_1", "Visit Harbor")
        .append_pair("nav_href_1", "/pages/visit-harbor")
        .append_pair("nav_label_2", "Member Guide")
        .append_pair("nav_href_2", "/pages/membership-guide")
        .finish();
    let save_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/navigation/save")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(save_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let save_status = save_response.status();
    let save_headers = save_response.headers().clone();
    let save_body_text = String::from_utf8(
        to_bytes(save_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(save_status, StatusCode::SEE_OTHER, "{save_body_text}");
    assert_eq!(
        save_headers.get("location").unwrap().to_str().unwrap(),
        "/admin/navigation"
    );

    let home_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let home_body = String::from_utf8(
        to_bytes(home_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(home_body.contains(">Visit Harbor<"), "{home_body}");
    assert!(home_body.contains("/pages/visit-harbor"), "{home_body}");
    assert!(home_body.contains(">Member Guide<"), "{home_body}");
    assert!(home_body.contains("/pages/membership-guide"), "{home_body}");

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Save navigation"), "{audit_body}");
    assert!(audit_body.contains("cms.navigation.edit"), "{audit_body}");
    assert!(
        audit_body.contains("navigation:primary-navigation"),
        "{audit_body}"
    );
    assert!(audit_body.contains("Succeeded"), "{audit_body}");
}

#[tokio::test]
async fn server_host_applies_checked_in_harbor_shop_redirect_rules_from_cms_admin() {
    let app_name = unique_app_name("harbor-shop-runtime-cms-redirects");
    let config = checked_in_harbor_shop_config(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/redirects")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let save_token = response_header(&admin_response, "x-davenda-cms-csrf-cms-redirects-save");

    let save_body = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &save_token)
        .append_pair("redirect_from_0", "/legacy/home")
        .append_pair("redirect_to_0", "/")
        .append_pair("redirect_permanent_0", "yes")
        .append_pair("new_redirect_from", "/legacy/about")
        .append_pair("new_redirect_to", "/pages/visit-harbor")
        .append_pair("new_redirect_permanent", "yes")
        .finish();
    let save_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/redirects/save")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(save_body))
                .unwrap(),
        )
        .await
        .unwrap();
    let save_status = save_response.status();
    let save_headers = save_response.headers().clone();
    let save_body_text = String::from_utf8(
        to_bytes(save_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(save_status, StatusCode::SEE_OTHER, "{save_body_text}");
    assert_eq!(
        save_headers.get("location").unwrap().to_str().unwrap(),
        "/admin/redirects"
    );

    let redirect_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/legacy/about")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(redirect_response.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        response_header(&redirect_response, "location"),
        "/pages/visit-harbor"
    );

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Save redirects"), "{audit_body}");
    assert!(
        audit_body.contains("redirects:live-redirect-rules"),
        "{audit_body}"
    );
    assert!(audit_body.contains("cms.page.edit"), "{audit_body}");
}

#[tokio::test]
async fn server_host_updates_checked_in_harbor_shop_catalog_from_admin_surface() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-catalog-update");
    let mut config = config_with_app_name(&app_name);
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_body = String::from_utf8(
        to_bytes(admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(admin_body.contains("Harbor Cap"), "{admin_body}");
    let product_token =
        storefront_csrf_token_from_body(&admin_body, "commerce.catalog-admin-update");
    assert!(
        admin_body.contains(&format!(r#"name="_csrf" value="{product_token}""#)),
        "{admin_body}"
    );

    let product_update = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &product_token)
        .append_pair("catalog_entity", "product")
        .append_pair("product_handle", "harbor-cap")
        .append_pair("product_title", "Dockside Cap")
        .append_pair(
            "product_summary",
            "Updated from Harbor Shop admin to prove live merchandising changes.",
        )
        .append_pair("product_price", "31.00")
        .append_pair("product_collection_handle", "featured")
        .append_pair("product_visible", "yes")
        .finish();
    let product_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(product_update))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(product_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&product_response, "location"),
        "/admin/catalog/products"
    );
    let flash_cookie = cookie_pair_from_response(&product_response, "davenda_flash")
        .expect("catalog admin save should set a flash cookie");

    let admin_cookie = format!("{session_cookie}; {flash_cookie}");
    let updated_admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let updated_admin_body = String::from_utf8(
        to_bytes(updated_admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        updated_admin_body.contains("Saved product changes for Dockside Cap."),
        "{updated_admin_body}"
    );
    assert!(
        updated_admin_body.contains("value=\"Dockside Cap\""),
        "{updated_admin_body}"
    );
    assert!(
        updated_admin_body
            .contains("Updated from Harbor Shop admin to prove live merchandising changes."),
        "{updated_admin_body}"
    );
    assert!(
        updated_admin_body.contains("value=\"31.00\""),
        "{updated_admin_body}"
    );

    let product_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/harbor-cap")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let product_page_body = String::from_utf8(
        to_bytes(product_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        product_page_body.contains("Dockside Cap"),
        "{product_page_body}"
    );
    assert!(
        product_page_body
            .contains("Updated from Harbor Shop admin to prove live merchandising changes."),
        "{product_page_body}"
    );
    assert!(product_page_body.contains("£31.00"), "{product_page_body}");

    let collection_token =
        storefront_csrf_token_from_body(&updated_admin_body, "commerce.catalog-admin-update");
    let collection_update = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &collection_token)
        .append_pair("catalog_entity", "collection")
        .append_pair("collection_handle", "featured")
        .append_pair("collection_title", "Harbor Essentials")
        .append_pair("collection_label", "Live catalog")
        .append_pair(
            "collection_summary",
            "Updated collection copy from the checked-in Harbor Shop admin route.",
        )
        .append_pair("collection_visible", "yes")
        .finish();
    let collection_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(collection_update))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(collection_response.status(), StatusCode::SEE_OTHER);
    let collection_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections/featured")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collection_page_body = String::from_utf8(
        to_bytes(collection_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        collection_page_body.contains("Harbor Essentials"),
        "{collection_page_body}"
    );
    assert!(
        collection_page_body
            .contains("Updated collection copy from the checked-in Harbor Shop admin route."),
        "{collection_page_body}"
    );

    let catalog_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let catalog_page_body = String::from_utf8(
        to_bytes(catalog_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        catalog_page_body.contains("Dockside Cap"),
        "{catalog_page_body}"
    );
    assert!(
        catalog_page_body.contains("Harbor Essentials"),
        "{catalog_page_body}"
    );
    assert!(
        catalog_page_body.contains("Live catalog"),
        "{catalog_page_body}"
    );

    let collections_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collections_page_body = String::from_utf8(
        to_bytes(collections_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        collections_page_body.contains("Harbor Essentials"),
        "{collections_page_body}"
    );
    assert!(
        collections_page_body.contains("Live catalog"),
        "{collections_page_body}"
    );

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Update product"), "{audit_body}");
    assert!(audit_body.contains("Update collection"), "{audit_body}");
    assert!(audit_body.contains("product:harbor-cap"), "{audit_body}");
    assert!(audit_body.contains("collection:featured"), "{audit_body}");
}

#[tokio::test]
async fn server_host_can_hide_products_and_collections_from_checked_in_harbor_shop_storefront() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-catalog-visibility");
    let mut config = config_with_app_name(&app_name);
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-1")
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);

    let admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_body = String::from_utf8(
        to_bytes(admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let product_token =
        storefront_csrf_token_from_body(&admin_body, "commerce.catalog-admin-update");
    let product_update = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &product_token)
        .append_pair("catalog_entity", "product")
        .append_pair("product_handle", "harbor-cap")
        .append_pair("product_title", "Harbor Cap")
        .append_pair(
            "product_summary",
            "A classic canvas cap with embroidered harbor mark.",
        )
        .append_pair("product_price", "29.00")
        .append_pair("product_collection_handle", "featured")
        .finish();
    let product_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(product_update))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(product_response.status(), StatusCode::SEE_OTHER);
    let flash_cookie = cookie_pair_from_response(&product_response, "davenda_flash")
        .expect("catalog admin save should set a flash cookie");
    let admin_cookie = format!("{session_cookie}; {flash_cookie}");

    let updated_admin_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let updated_admin_body = String::from_utf8(
        to_bytes(updated_admin_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        updated_admin_body.contains("Hidden from storefront"),
        "{updated_admin_body}"
    );

    let catalog_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let catalog_page_body = String::from_utf8(
        to_bytes(catalog_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        !catalog_page_body.contains("Harbor Cap"),
        "{catalog_page_body}"
    );

    let hidden_product_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/products/harbor-cap")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let hidden_product_body = String::from_utf8(
        to_bytes(hidden_product_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        hidden_product_body.contains("This product is not currently available."),
        "{hidden_product_body}"
    );
    assert!(
        !hidden_product_body.contains("Add to cart"),
        "{hidden_product_body}"
    );

    let collection_token =
        storefront_csrf_token_from_body(&updated_admin_body, "commerce.catalog-admin-update");
    let collection_update = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("_csrf", &collection_token)
        .append_pair("catalog_entity", "collection")
        .append_pair("collection_handle", "featured")
        .append_pair("collection_title", "Hidden Seasonal Edit")
        .append_pair("collection_label", "Dormant")
        .append_pair(
            "collection_summary",
            "Temporarily removed from the live Harbor Shop browse path.",
        )
        .finish();
    let collection_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/catalog/products")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(collection_update))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(collection_response.status(), StatusCode::SEE_OTHER);

    let collections_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let collections_page_body = String::from_utf8(
        to_bytes(collections_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        !collections_page_body.contains("Hidden Seasonal Edit"),
        "{collections_page_body}"
    );

    let hidden_collection_page = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/en-GB/shop/collections/featured")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let hidden_collection_body = String::from_utf8(
        to_bytes(hidden_collection_page.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        hidden_collection_body.contains("This collection is not currently available."),
        "{hidden_collection_body}"
    );
}

#[tokio::test]
async fn server_host_renders_live_completed_orders_on_checked_in_admin_orders_surface() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-live-orders");
    let mut config = with_payment_webhook_secret(config_with_app_name(&app_name));
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan.clone(),
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let principal_id = "operator-live-orders";
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal(principal_id)
                .unwrap(),
            now,
        )
        .unwrap();
    let session_cookie = format!("davenda_session={}", issued.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &issued.record.session_id,
            Some(principal_id),
            "gold-membership",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(&issued.record.session_id, Some(principal_id), 101)
        .unwrap();
    store
        .checkout_complete(
            &issued.record.session_id,
            Some(principal_id),
            &StorefrontPaymentInput::card("operator-live-orders@example.com", "4242", "PAY-50001")
                .unwrap(),
            102,
        )
        .unwrap();
    let response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("1 completed orders are available for operator review."),
        "{body}"
    );

    let response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &session_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("ORD-10042"), "{body}");
    assert!(body.contains("Pending Payment"), "{body}");
    assert!(body.contains("£89.00"), "{body}");
    assert!(!body.contains("No completed orders yet"), "{body}");
    assert!(body.contains("View details"), "{body}");
    assert!(body.contains("operator-live-orders@example.com"), "{body}");
}

#[tokio::test]
async fn server_host_supports_checked_in_harbor_shop_order_detail_and_refund_flow() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-order-detail-refund");
    let mut config = with_payment_webhook_secret(config_with_app_name(&app_name));
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan.clone(),
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let customer_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-customer-order-support")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-order-support")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_cookie = format!("davenda_session={}", operator_session.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &customer_session.record.session_id,
            Some("member-live-customer-order-support"),
            "gold-membership",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(
            &customer_session.record.session_id,
            Some("member-live-customer-order-support"),
            101,
        )
        .unwrap();
    store
        .checkout_complete(
            &customer_session.record.session_id,
            Some("member-live-customer-order-support"),
            &StorefrontPaymentInput::card("buyer@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();
    store
        .apply_payment_webhook("PAY-50001", "payment.captured", 103)
        .unwrap();

    let admin_orders_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_orders_body = String::from_utf8(
        to_bytes(admin_orders_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        admin_orders_body.contains("ORD-10042"),
        "{admin_orders_body}"
    );
    assert!(
        admin_orders_body.contains("buyer@example.com"),
        "{admin_orders_body}"
    );
    assert!(
        admin_orders_body.contains("View details"),
        "{admin_orders_body}"
    );

    let detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = String::from_utf8(
        to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(detail_body.contains("buyer@example.com"), "{detail_body}");
    assert!(detail_body.contains("PAY-50001"), "{detail_body}");
    assert!(detail_body.contains("Gold Membership"), "{detail_body}");
    assert!(detail_body.contains("Issue refund"), "{detail_body}");
    let refund_token = storefront_csrf_token_from_body(&detail_body, "commerce.order-refund");

    let refund_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/orders/refund")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("_csrf", &refund_token)
                        .append_pair("order_id", "ORD-10042")
                        .append_pair("reason", "customer_support")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refund_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&refund_response, "location"),
        "/admin/orders/ORD-10042"
    );
    let flash_cookie = cookie_pair_from_response(&refund_response, "davenda_flash")
        .expect("refund flow should set a flash cookie");
    let refund_cookie = format!("{operator_cookie}; {flash_cookie}");

    let refunded_detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &refund_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let refunded_detail_body = String::from_utf8(
        to_bytes(refunded_detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        refunded_detail_body.contains("Refunded £89.00 for order ORD-10042."),
        "{refunded_detail_body}"
    );
    assert!(
        refunded_detail_body.contains("Refunded"),
        "{refunded_detail_body}"
    );
    assert!(
        refunded_detail_body.contains("RFD-07001"),
        "{refunded_detail_body}"
    );
    assert!(
        refunded_detail_body.contains("customer_support"),
        "{refunded_detail_body}"
    );

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Issue refund"), "{audit_body}");
    assert!(audit_body.contains("order:ORD-10042"), "{audit_body}");
    assert!(audit_body.contains("customer_support"), "{audit_body}");
}

#[tokio::test]
async fn server_host_supports_checked_in_harbor_shop_order_fulfillment_flow() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-order-detail-fulfill");
    let mut config = with_payment_webhook_secret(config_with_app_name(&app_name));
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_module(davenda_memberships::MembershipsModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan.clone(),
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let customer_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-customer-order-fulfillment")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-order-fulfillment")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_cookie = format!("davenda_session={}", operator_session.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &customer_session.record.session_id,
            Some("member-live-customer-order-fulfillment"),
            "gold-membership",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(
            &customer_session.record.session_id,
            Some("member-live-customer-order-fulfillment"),
            101,
        )
        .unwrap();
    store
        .checkout_complete(
            &customer_session.record.session_id,
            Some("member-live-customer-order-fulfillment"),
            &StorefrontPaymentInput::card("fulfillment@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();
    store
        .apply_payment_webhook("PAY-50001", "payment.captured", 103)
        .unwrap();

    let detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);
    let detail_body = String::from_utf8(
        to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(detail_body.contains("Mark fulfilled"), "{detail_body}");
    assert!(detail_body.contains("Fulfillment action"), "{detail_body}");
    let fulfill_token = storefront_csrf_token_from_body(&detail_body, "commerce.order-fulfill");

    let fulfill_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/orders/fulfill")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("_csrf", &fulfill_token)
                        .append_pair("order_id", "ORD-10042")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fulfill_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&fulfill_response, "location"),
        "/admin/orders/ORD-10042"
    );
    let flash_cookie = cookie_pair_from_response(&fulfill_response, "davenda_flash")
        .expect("fulfillment flow should set a flash cookie");
    let fulfill_cookie = format!("{operator_cookie}; {flash_cookie}");

    let fulfilled_detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &fulfill_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let fulfilled_detail_body = String::from_utf8(
        to_bytes(fulfilled_detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        fulfilled_detail_body.contains("Marked order ORD-10042 as fulfilled."),
        "{fulfilled_detail_body}"
    );
    assert!(
        fulfilled_detail_body.contains("Fulfilled"),
        "{fulfilled_detail_body}"
    );
    assert!(
        !fulfilled_detail_body.contains("Mark fulfilled"),
        "{fulfilled_detail_body}"
    );

    let admin_orders_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admin_orders_body = String::from_utf8(
        to_bytes(admin_orders_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        admin_orders_body.contains("Fulfilled"),
        "{admin_orders_body}"
    );

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Mark fulfilled"), "{audit_body}");
    assert!(audit_body.contains("order:ORD-10042"), "{audit_body}");
}

#[tokio::test]
async fn server_host_replays_refund_validation_errors_on_checked_in_order_detail() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-order-refund-validation");
    let mut config = with_payment_webhook_secret(config_with_app_name(&app_name));
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan.clone(),
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let customer_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-customer-order-refund-validation")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-order-refund-validation")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_cookie = format!("davenda_session={}", operator_session.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &customer_session.record.session_id,
            Some("member-live-customer-order-refund-validation"),
            "gold-membership",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(
            &customer_session.record.session_id,
            Some("member-live-customer-order-refund-validation"),
            101,
        )
        .unwrap();
    store
        .checkout_complete(
            &customer_session.record.session_id,
            Some("member-live-customer-order-refund-validation"),
            &StorefrontPaymentInput::card("refund-validation@example.com", "4242", "PAY-50001")
                .unwrap(),
            102,
        )
        .unwrap();
    store
        .apply_payment_webhook("PAY-50001", "payment.captured", 103)
        .unwrap();

    let detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail_body = String::from_utf8(
        to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let refund_token = storefront_csrf_token_from_body(&detail_body, "commerce.order-refund");

    let refund_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/orders/refund")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("_csrf", &refund_token)
                        .append_pair("order_id", "ORD-10042")
                        .append_pair("reason", "")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refund_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&refund_response, "location"),
        "/admin/orders/ORD-10042"
    );
    let flash_cookie = cookie_pair_from_response(&refund_response, "davenda_flash")
        .expect("refund validation should set a flash cookie");
    let refund_cookie = format!("{operator_cookie}; {flash_cookie}");

    let invalid_detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &refund_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let invalid_detail_body = String::from_utf8(
        to_bytes(invalid_detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        invalid_detail_body
            .contains("Review the refund request and add a reason before trying again."),
        "{invalid_detail_body}"
    );
    assert!(
        invalid_detail_body.contains("refund reason is required"),
        "{invalid_detail_body}"
    );
    assert!(
        invalid_detail_body.contains(r#"name="reason" value="""#),
        "{invalid_detail_body}"
    );

    let audit_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/audit")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_body = String::from_utf8(
        to_bytes(audit_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(audit_body.contains("Issue refund"), "{audit_body}");
    assert!(audit_body.contains("Rejected"), "{audit_body}");
    assert!(
        audit_body.contains("Refund reason was required but not provided."),
        "{audit_body}"
    );
}

#[tokio::test]
async fn server_host_explains_pending_payment_refund_block_on_order_detail() {
    let app_name = unique_app_name("harbor-shop-runtime-admin-order-refund-block");
    let mut config = with_payment_webhook_secret(config_with_app_name(&app_name));
    config.auth.package = "harbor-auth".to_string();
    let template_root = checked_in_harbor_shop_root();
    let auth_package = davenda_auth::load_auth_model_package_at("harbor-auth", &template_root)
        .expect("checked-in harbor auth package should load");
    let plan = RuntimeBuilder::new(config, auth_package)
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(davenda_commerce::CommercePaymentsStripeModule::new())
        .with_template_root(&template_root)
        .build()
        .unwrap();
    let resolver = live_backend_secret_resolver_with_payment_webhook();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let server = HttpServerHost::new_with_authorizer(
        plan.clone(),
        backends,
        b"01234567012345670123456701234567".to_vec(),
        b"76543210765432107654321076543210".to_vec(),
        Arc::new(PermissiveLiveRouteCapabilityAuthorizer),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let customer_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-customer-order-refund-block")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_session = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("operator-live-order-refund-block")
                .unwrap(),
            now,
        )
        .unwrap();
    let operator_cookie = format!("davenda_session={}", operator_session.cookie_value);
    let store = StorefrontStateStore::open_for_plan(&plan).unwrap();
    store
        .add_to_cart(
            &customer_session.record.session_id,
            Some("member-live-customer-order-refund-block"),
            "gold-membership",
            1,
            100,
        )
        .unwrap();
    store
        .checkout_start(
            &customer_session.record.session_id,
            Some("member-live-customer-order-refund-block"),
            101,
        )
        .unwrap();
    store
        .checkout_complete(
            &customer_session.record.session_id,
            Some("member-live-customer-order-refund-block"),
            &StorefrontPaymentInput::card("refund-block@example.com", "4242", "PAY-50001").unwrap(),
            102,
        )
        .unwrap();

    let detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail_body = String::from_utf8(
        to_bytes(detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    let refund_token = storefront_csrf_token_from_body(&detail_body, "commerce.order-refund");
    assert!(detail_body.contains("Refund unavailable"), "{detail_body}");
    assert!(
        detail_body.contains("awaiting provider confirmation"),
        "{detail_body}"
    );

    let refund_response = server
        .respond(
            Request::builder()
                .method("POST")
                .uri("/admin/orders/refund")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &operator_cookie)
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(
                    url::form_urlencoded::Serializer::new(String::new())
                        .append_pair("_csrf", &refund_token)
                        .append_pair("order_id", "ORD-10042")
                        .append_pair("reason", "customer_support")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refund_response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response_header(&refund_response, "location"),
        "/admin/orders/ORD-10042"
    );
    let flash_cookie = cookie_pair_from_response(&refund_response, "davenda_flash")
        .expect("refund block should set a flash cookie");
    let blocked_cookie = format!("{operator_cookie}; {flash_cookie}");

    let blocked_detail_response = server
        .respond(
            Request::builder()
                .method("GET")
                .uri("/admin/orders/ORD-10042")
                .header("host", "www.example.com")
                .header("x-forwarded-proto", "https")
                .header("cookie", &blocked_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let blocked_detail_body = String::from_utf8(
        to_bytes(blocked_detail_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(
        blocked_detail_body.contains(
            "This order cannot be refunded from the checked-in admin workflow right now."
        ),
        "{blocked_detail_body}"
    );
    assert!(
        blocked_detail_body.contains("cannot be refunded while it is `pending_payment`"),
        "{blocked_detail_body}"
    );
    assert!(
        blocked_detail_body.contains(r#"name="reason" value="customer_support""#),
        "{blocked_detail_body}"
    );
}

#[tokio::test]
async fn server_host_executes_api_extensions_during_live_requests() {
    let extension_dir = unique_temp_extension_dir("api-wasm");
    fs::create_dir_all(&extension_dir).unwrap();
    let config = config_with_extension_directory(&extension_dir);
    let api_slots = StaticManifestModule::new(
        ModuleManifest::new("api.runtime.slot").with_extension_slots(vec![
            ExtensionSlotDescriptor::new(
                ExtensionSlotKind::Api,
                "/api/account",
                "Allows API extensions to participate in the live request path",
            ),
        ]),
    );
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(api_slots)
        .with_route(
            RouteDefinition::new("account.api", HttpMethod::Get, "/api/account")
                .unwrap()
                .with_area(RouteArea::Api)
                .requiring_session(),
        )
        .with_handler(
            HandlerDefinition::json(
                "account.api",
                BTreeMap::from([("status".to_string(), "ok".to_string())]),
            )
            .unwrap(),
        )
        .with_installed_extension(installed_api_extension_with_artifact(
            &extension_dir,
            "/api/account",
            "showcase-events",
        ))
        .build()
        .unwrap();

    let resolver = live_backend_secret_resolver();
    let server = plan
        .server_host(
            &resolver,
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-live-4")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/api/account")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers.get("x-davenda-wasm-request-handler").unwrap(),
        "account-json"
    );
    assert_eq!(
        headers.get("x-davenda-wasm-request-outcome").unwrap(),
        "ApiJson"
    );
    assert!(body.contains("\"status\":\"ok\""));
    assert!(body.contains("\"extension\":\"ok\""));

    fs::remove_dir_all(&extension_dir).unwrap();
}

#[tokio::test]
async fn server_host_rejects_capability_routes_when_live_authorizer_denies() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .build()
        .unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let resolver = live_backend_secret_resolver();
    let backends = plan.shared_backend_clients(&resolver).unwrap();
    let authorizer = Arc::new(StaticLiveRouteCapabilityAuthorizer::new());
    let server = HttpServerHost::new_with_authorizer(
        plan,
        backends,
        cookie_secret.to_vec(),
        csrf_secret.to_vec(),
        authorizer.clone(),
    )
    .unwrap();
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let issued = server
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("editor-live-2")
                .unwrap(),
            now,
        )
        .unwrap();
    let request = Request::builder()
        .method("GET")
        .uri("/admin/pages/preview")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("cookie", format!("davenda_session={}", issued.cookie_value))
        .body(Body::empty())
        .unwrap();

    let response = server.respond(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        authorizer.checks(),
        vec![LiveAuthorizationCheck {
            subject: davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(
                "editor-live-2",
            )),
            capability: Capability::CmsPageRead,
            object: davenda_auth::Entity::page("http.surface.module.cms.page.cms.preview"),
        }]
    );
}
