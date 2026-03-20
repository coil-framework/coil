use super::*;

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
                .body(Body::from(vec![b'x'; 16]))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
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
