use super::*;
use crate::RuntimeBuilder;
use davenda_auth::DefaultAuthModelPackage;
use davenda_config::PlatformConfig;
use davenda_wasm::{
    CustomerAppContext, ExtensionId, HandlerId, HostCapabilityGrant, HostGrantSet, HttpMethod,
    InvocationContext, InvocationInput, InvocationPlan, JobExecution, MetadataExecution,
    MetadataGrant, NetworkExecution, PageInvocation, PrincipalRef, ResourceLimits, SecretExecution,
    TraceContext,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use url::Url;

const TEST_CONFIG: &str = r#"
[app]
name = "wasm-host-tests"
environment = "development"

[server]
bind = "127.0.0.1:0"
trusted_proxies = []

[http.session]
store = "memory"
idle_timeout_secs = 3600
absolute_timeout_secs = 7200

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = false
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "disabled"
deployment = "distributed"
local_root = "/tmp/davenda-runtime-tests"

[cache]
l1 = "moka"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"
localized_routes = false

[seo]
canonical_host = "example.test"
emit_json_ld = false

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms-pages"]

[wasm]
directory = "/tmp/davenda-wasm-tests"
default_time_limit_ms = 50
allow_network = true

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#;

#[test]
fn runtime_host_service_executor_uses_live_backends() {
    let plan = RuntimeBuilder::new(
        PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
        DefaultAuthModelPackage::default(),
    )
    .build()
    .unwrap();
    let (endpoint, server) = spawn_http_server("live-response");

    let mut http_targets = BTreeMap::new();
    http_targets.insert("crm".to_string(), Url::parse(&endpoint).unwrap());
    let mut secrets = BTreeMap::new();
    secrets.insert("api-token".to_string(), "super-secret".to_string());

    let shared_root = shared_state_root("metadata");
    let services = RuntimeWasmHostServices::with_shared_state_root(
        shared_root.clone(),
        plan.clone(),
        http_targets,
        secrets,
    );
    let executor = RuntimeHostServiceExecutor::with_services(plan.clone(), services.clone());
    let session_plan = InvocationPlan {
        extension_id: ExtensionId::new("extensions.live").unwrap(),
        handler_id: HandlerId::new("handler-live").unwrap(),
        point: ExtensionPointKind::Page,
        customer_app_id: "customer-app".to_string(),
        granted_capabilities: HostGrantSet::from_grants([
            HostCapabilityGrant::OutboundHttp {
                integration: "crm".to_string(),
            },
            HostCapabilityGrant::SecretRead {
                secret: "api-token".to_string(),
            },
            HostCapabilityGrant::EnqueueJob {
                queue: "jobs.work".to_string(),
            },
            HostCapabilityGrant::MetadataWrite {
                kind: MetadataGrant::JsonLd,
            },
        ]),
        limits: ResourceLimits::baseline_for(ExtensionPointKind::Page),
        context: InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-1").unwrap(),
            TraceContext::new("trace-1").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/host-side-effects", HttpMethod::Get).unwrap(),
            ),
        ),
    };

    let mut session = session_plan.begin_execution_with_executor(Arc::new(executor));

    let network = session
        .execute_host_call(davenda_wasm::HostCall::OutboundHttp {
            integration: "crm".to_string(),
            response_bytes: "live-response".len() as u64,
        })
        .unwrap();
    assert!(matches!(
        network.result,
        HostServiceResult::Network(NetworkExecution {
            integration,
            endpoint: recorded_endpoint,
            status,
            response_bytes,
        }) if integration == "crm"
            && recorded_endpoint == Url::parse(&endpoint).unwrap().to_string()
            && status == 200
            && response_bytes == "live-response".len() as u64
    ));
    assert_eq!(
        session.usage().outbound_response_bytes,
        "live-response".len() as u64
    );

    let secret = session
        .execute_host_call(davenda_wasm::HostCall::SecretRead {
            secret: "api-token".to_string(),
        })
        .unwrap();
    assert!(matches!(
        secret.result,
        HostServiceResult::Secret(SecretExecution {
            secret,
            source,
            value_bytes,
        }) if secret == "api-token"
            && source == "in-memory:api-token"
            && value_bytes == "super-secret".len()
    ));

    let job = session
        .execute_host_call(davenda_wasm::HostCall::EnqueueJob {
            queue: "jobs.work".to_string(),
        })
        .unwrap();
    assert!(matches!(
        job.result,
        HostServiceResult::Job(JobExecution {
            queue,
            job_id,
            enqueued_at_unix_seconds,
        }) if queue == "jobs.work"
            && job_id.starts_with("wasm:")
            && enqueued_at_unix_seconds > 0
    ));

    let jobs = plan.jobs_host("scheduler-a").unwrap();
    assert_eq!(jobs.coordinator().ready_jobs().len(), 1);
    assert_eq!(
        jobs.coordinator().ready_jobs()[0].spec.queue.as_str(),
        "jobs.work"
    );

    let metadata = session
        .execute_host_call(davenda_wasm::HostCall::MetadataWrite {
            kind: MetadataGrant::JsonLd,
        })
        .unwrap();
    assert!(matches!(
        metadata.result,
        HostServiceResult::Metadata(MetadataExecution {
            kind: MetadataGrant::JsonLd,
            recorded: true,
            journal_entries: 1,
        })
    ));

    let metadata = session
        .execute_host_call(davenda_wasm::HostCall::MetadataWrite {
            kind: MetadataGrant::JsonLd,
        })
        .unwrap();
    assert!(matches!(
        metadata.result,
        HostServiceResult::Metadata(MetadataExecution {
            kind: MetadataGrant::JsonLd,
            recorded: true,
            journal_entries: 2,
        })
    ));
    let reopened = RuntimeWasmHostServices::with_shared_state_root(
        shared_root,
        plan.clone(),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let snapshot = reopened.metadata_snapshot(10).unwrap();
    assert_eq!(snapshot.entry_count, 2);
    assert_eq!(snapshot.recent_records.len(), 2);
    assert!(snapshot.path.as_ref().unwrap().exists());
    assert_eq!(snapshot.recent_records[0].kind, "json_ld");
    assert_eq!(snapshot.recent_records[0].trace_id, "trace-1");
    assert_eq!(snapshot.recent_records[0].app_id, "customer-app");
    assert_eq!(snapshot.recent_records[1].kind, "json_ld");
    assert_eq!(snapshot.recent_records[1].trace_id, "trace-1");
    assert_eq!(snapshot.recent_records[1].app_id, "customer-app");

    server.join().unwrap();
}

#[test]
fn runtime_host_service_executor_uses_runtime_scoped_secret_bindings() {
    let plan = RuntimeBuilder::new(
        PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
        DefaultAuthModelPackage::default(),
    )
    .build()
    .unwrap();

    let mut secrets = BTreeMap::new();
    secrets.insert("api-token".to_string(), "super-secret".to_string());

    let services = RuntimeWasmHostServices::with_runtime_secrets(plan.clone(), secrets);
    let executor = RuntimeHostServiceExecutor::with_services(plan.clone(), services.clone());
    let session_plan = InvocationPlan {
        extension_id: ExtensionId::new("extensions.live").unwrap(),
        handler_id: HandlerId::new("handler-live").unwrap(),
        point: ExtensionPointKind::Page,
        customer_app_id: "customer-app".to_string(),
        granted_capabilities: HostGrantSet::from_grants([HostCapabilityGrant::SecretRead {
            secret: "api-token".to_string(),
        }]),
        limits: ResourceLimits::baseline_for(ExtensionPointKind::Page),
        context: InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-1").unwrap(),
            TraceContext::new("trace-1").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/host-side-effects", HttpMethod::Get).unwrap(),
            ),
        ),
    };

    let mut session = session_plan.begin_execution_with_executor(Arc::new(executor));
    let secret = session
        .execute_host_call(davenda_wasm::HostCall::SecretRead {
            secret: "api-token".to_string(),
        })
        .unwrap();

    assert!(matches!(
        secret.result,
        HostServiceResult::Secret(SecretExecution {
            secret,
            source,
            value_bytes,
        }) if secret == "api-token"
            && source == "runtime:wasm-host-tests:api-token"
            && value_bytes == "super-secret".len()
    ));
}

#[test]
fn runtime_host_service_executor_denies_unbound_secrets_without_env_fallback() {
    let plan = RuntimeBuilder::new(
        PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
        DefaultAuthModelPackage::default(),
    )
    .build()
    .unwrap();

    let services = RuntimeWasmHostServices::new(plan.clone());
    let executor = RuntimeHostServiceExecutor::with_services(plan.clone(), services);
    let session_plan = InvocationPlan {
        extension_id: ExtensionId::new("extensions.live").unwrap(),
        handler_id: HandlerId::new("handler-live").unwrap(),
        point: ExtensionPointKind::Page,
        customer_app_id: "customer-app".to_string(),
        granted_capabilities: HostGrantSet::from_grants([HostCapabilityGrant::SecretRead {
            secret: "api-token".to_string(),
        }]),
        limits: ResourceLimits::baseline_for(ExtensionPointKind::Page),
        context: InvocationContext::new(
            CustomerAppContext::new("customer-app").unwrap(),
            PrincipalRef::user("user-1").unwrap(),
            TraceContext::new("trace-1").unwrap(),
            InvocationInput::Page(
                PageInvocation::new("/host-side-effects", HttpMethod::Get).unwrap(),
            ),
        ),
    };

    let mut session = session_plan.begin_execution_with_executor(Arc::new(executor));
    let error = session
        .execute_host_call(davenda_wasm::HostCall::SecretRead {
            secret: "api-token".to_string(),
        })
        .unwrap_err();

    assert!(format!("{error:?}").contains("was not provided to runtime"));
}

fn shared_state_root(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "davenda-wasm-host-{}-{}",
        std::process::id(),
        label
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn spawn_http_server(body: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream.write_all(response.as_bytes()).unwrap();
    });

    (endpoint, handle)
}
