use super::*;
use axum::response::Response;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Mutex;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostedCheckoutCall {
    api_key: String,
    request_body: String,
    idempotency_key: String,
}

#[derive(Debug)]
struct StaticHostedCheckoutClient {
    session_id: String,
    session_url: String,
    calls: Mutex<Vec<HostedCheckoutCall>>,
}

impl StaticHostedCheckoutClient {
    fn with_url(session_url: &str) -> Self {
        Self {
            session_id: "cs_test_harbor_shop".to_string(),
            session_url: session_url.to_string(),
            calls: Mutex::new(Vec::new()),
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
        self.calls.lock().unwrap().push(HostedCheckoutCall {
            api_key: api_key.to_string(),
            request_body: request_body.to_string(),
            idempotency_key: idempotency_key.to_string(),
        });
        Ok(crate::server::HostedCheckoutSession {
            id: self.session_id.clone(),
            url: self.session_url.clone(),
        })
    }
}

fn live_backend_secret_resolver() -> StaticSecretResolver {
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
            LIVE_OBJECT_STORE_SECRET,
        )
        .unwrap()
}

fn live_backend_secret_resolver_with_payment_webhook() -> StaticSecretResolver {
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
            STRIPE_SECRET_KEY,
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
    response
        .headers()
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
        .with_route(
            RouteDefinition::new("storefront.home", HttpMethod::Get, "/")
                .unwrap()
                .from_module("commerce"),
        )
        .with_handler(HandlerDefinition::page("storefront.home", "pages/home").unwrap())
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
    assert!(body.contains("storefront.home"), "{body}");
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
                .body(Body::from(webhook_body))
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
    assert!(checkout_body.contains("Continue to Stripe"), "{checkout_body}");

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
                .uri("/checkout/confirmation?provider_result=return&payment_reference=PAY-50001")
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
    assert!(confirmation_body.contains("Stripe hosted checkout"), "{confirmation_body}");

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
        admin_orders_body.contains("Gold Membership"),
        "{admin_orders_body}"
    );
    assert!(admin_orders_body.contains("£89.00"), "{admin_orders_body}");
    assert!(
        admin_orders_body.contains("Pending Payment"),
        "{admin_orders_body}"
    );
    assert!(
        admin_orders_body.contains("returned from Stripe"),
        "{admin_orders_body}"
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
    assert!(checkout_body.contains("Continue to Stripe"), "{checkout_body}");
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
            .contains("success_url=http%3A%2F%2Fwww.example.com%2Fcheckout%2Fconfirmation%3Fprovider_result%3Dreturn%26payment_reference%3DPAY-50001"),
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
                .uri("/checkout/confirmation?provider_result=return&payment_reference=PAY-50001")
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
    assert!(checkout_body.contains("Continue to Stripe"), "{checkout_body}");

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
                .uri("/checkout/confirmation?provider_result=return&payment_reference=PAY-50001")
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
    let mut config = config_with_app_name("harbor-shop");
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
                assert!(body.contains("Order support baseline"), "{route}: {body}");
                assert!(body.contains("Cutover content checks"), "{route}: {body}");
            }
            "/admin/orders" => {
                assert!(body.contains("Settlement boundary"), "{route}: {body}");
                assert!(
                    body.contains("does not yet expose payment references"),
                    "{route}: {body}"
                );
                assert!(
                    body.contains("Customer-visible consistency"),
                    "{route}: {body}"
                );
            }
            "/admin/catalog/products" => {
                assert!(body.contains("Sellable truth"), "{route}: {body}");
                assert!(
                    body.contains("Inventory mutation, product editing, and publish workflows"),
                    "{route}: {body}"
                );
            }
            "/admin/pages" => {
                assert!(body.contains("Launch check"), "{route}: {body}");
                assert!(body.contains("route truth only"), "{route}: {body}");
            }
            "/admin/navigation" => {
                assert!(body.contains("navigation editor"), "{route}: {body}");
            }
            "/admin/redirects" => {
                assert!(
                    body.contains("tested redirect inventory"),
                    "{route}: {body}"
                );
            }
            _ => {}
        }
    }
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
