use super::*;
use axum::response::Response;

const LIVE_DATABASE_URL: &str = "postgres://platform:secret@db.internal/platform";
const LIVE_OBJECT_STORE_SECRET: &str = r#"
endpoint_url = "https://s3.internal"
bucket = "runtime"
region = "eu-west-2"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
signed_url_ttl_secs = 900
"#;

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
    let resolver = live_backend_secret_resolver();
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
    let resolver = live_backend_secret_resolver();
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
            &StorefrontPaymentInput::card("member-live-order-1@example.com", "4242").unwrap(),
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
        confirmation_body.contains("membership activation"),
        "{confirmation_body}"
    );
    assert!(confirmation_body.contains("Paid"), "{confirmation_body}");
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
    assert!(account_body.contains("Paid"), "{account_body}");
    assert!(account_body.contains("£118.00"), "{account_body}");
    assert!(account_body.contains("Gold Membership"), "{account_body}");
    assert!(account_body.contains("Purchased"), "{account_body}");
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
        confirmation_body.contains("fulfillment summary"),
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
    assert_eq!(payment_status, "captured");
    assert_eq!(payment_reference, "PAY-50001");
    assert!(
        confirmation_body.contains("\"status\":\"captured\""),
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
      <h1>Checkout</h1>
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
                        .append_pair("email", "buyer@example.com")
                        .finish(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = complete_response.status();
    let body = String::from_utf8(
        to_bytes(complete_response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();

    fs::remove_dir_all(&template_root).unwrap();

    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.contains("payment method"), "{body}");
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
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CommerceModule::new())
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
            &StorefrontPaymentInput::card("member@example.com", "4242").unwrap(),
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
    assert_eq!(account_response.status(), StatusCode::FOUND);
    assert_eq!(
        account_response.headers().get("location").unwrap(),
        "/account/memberships"
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
    assert!(memberships_body.contains("Purchased"), "{memberships_body}");
    assert!(
        memberships_body.contains("member@example.com"),
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
}

#[tokio::test]
async fn server_host_renders_checked_in_harbor_shop_catalog_collection_and_product_routes() {
    let app_name = unique_app_name("harbor-shop-runtime-catalog-routes");
    let config = config_with_app_name(&app_name);
    let template_root = checked_in_harbor_shop_root();
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
    assert!(headers
        .get("surrogate-key")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("account-runtime"));
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
    assert!(headers
        .get("surrogate-key")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("account-runtime"));
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
