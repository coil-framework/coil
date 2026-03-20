use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use url::Url;

use super::super::super::super::*;
use super::RuntimeOutboundHttpBackend;

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        unsafe {
            match self.previous.take() {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

fn execution_context() -> InvocationContext {
    InvocationContext::new(
        CustomerAppContext::new("showcase-events")
            .unwrap()
            .with_tenant_id("101")
            .unwrap()
            .with_locale("en-GB")
            .unwrap(),
        PrincipalRef::user("alice").unwrap(),
        TraceContext::new("trace-network").unwrap(),
        InvocationInput::Api(
            ApiInvocation::new("/network", davenda_wasm::HttpMethod::Get).unwrap(),
        ),
    )
}

fn spawn_http_server(
    body: &'static str,
    delay: Option<Duration>,
) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0u8; 1024];
        let _ = stream.read(&mut request);
        if let Some(delay) = delay {
            std::thread::sleep(delay);
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    });

    (endpoint, handle)
}

#[test]
fn runtime_outbound_http_backend_requires_explicit_mappings() {
    let backend = RuntimeOutboundHttpBackend::with_settings(
        true,
        BTreeMap::new(),
        Duration::from_millis(100),
        1024,
    );

    let error = backend
        .submit_outbound_http_to_blocking_pool("http://127.0.0.1:8080", 64, &execution_context())
        .unwrap_err();

    assert!(error.contains("not mapped"), "unexpected error: {error}");
}

#[test]
fn runtime_outbound_http_backend_ignores_environment_fallbacks() {
    let env_key = "DAVENDA_WASM_HTTP_NO_FALLBACK";
    let previous = std::env::var_os(env_key);
    unsafe {
        std::env::set_var(env_key, "https://env.example.com/api");
    }
    let _guard = EnvVarGuard {
        key: env_key,
        previous,
    };

    let backend = RuntimeOutboundHttpBackend::with_settings(
        true,
        BTreeMap::new(),
        Duration::from_millis(100),
        1024,
    );

    let error = backend
        .submit_outbound_http_to_blocking_pool("no-fallback", 64, &execution_context())
        .unwrap_err();

    assert!(error.contains("not mapped"), "unexpected error: {error}");
}

#[test]
fn runtime_outbound_http_backend_uses_explicit_endpoint_mappings() {
    let (endpoint, server) = spawn_http_server("mapped-response", None);
    let mut targets = BTreeMap::new();
    targets.insert("crm".to_string(), Url::parse(&endpoint).unwrap());

    let backend =
        RuntimeOutboundHttpBackend::with_settings(true, targets, Duration::from_secs(1), 1024);
    let execution = backend
        .submit_outbound_http_to_blocking_pool("crm", 64, &execution_context())
        .expect("mapped endpoint should succeed");

    assert_eq!(execution.integration, "crm");
    assert_eq!(
        execution.endpoint,
        Url::parse(&endpoint).unwrap().to_string()
    );
    assert_eq!(execution.status, 200);
    assert_eq!(execution.response_bytes, "mapped-response".len() as u64);

    server.join().unwrap();
}

#[test]
fn runtime_outbound_http_backend_enforces_response_size_limits() {
    let (endpoint, server) = spawn_http_server("too-many-bytes", None);
    let mut targets = BTreeMap::new();
    targets.insert("search".to_string(), Url::parse(&endpoint).unwrap());

    let backend =
        RuntimeOutboundHttpBackend::with_settings(true, targets, Duration::from_secs(1), 1024);
    let error = backend
        .submit_outbound_http_to_blocking_pool("search", 4, &execution_context())
        .unwrap_err();

    assert!(
        error.contains("exceeded the configured limit"),
        "unexpected error: {error}"
    );

    server.join().unwrap();
}

#[test]
fn runtime_outbound_http_backend_times_out_slow_endpoints() {
    let (endpoint, server) = spawn_http_server("slow-response", Some(Duration::from_millis(200)));
    let mut targets = BTreeMap::new();
    targets.insert("billing".to_string(), Url::parse(&endpoint).unwrap());

    let backend =
        RuntimeOutboundHttpBackend::with_settings(true, targets, Duration::from_millis(25), 1024);
    let error = backend
        .submit_outbound_http_to_blocking_pool("billing", 64, &execution_context())
        .unwrap_err();

    assert!(error.contains("timed out"), "unexpected error: {error}");

    server.join().unwrap();
}

#[test]
fn runtime_outbound_http_backend_yields_worker_threads_while_waiting() {
    let (endpoint, server) = spawn_http_server("slow-response", Some(Duration::from_millis(150)));
    let mut targets = BTreeMap::new();
    targets.insert("billing".to_string(), Url::parse(&endpoint).unwrap());

    let backend =
        RuntimeOutboundHttpBackend::with_settings(true, targets, Duration::from_millis(500), 1024);
    let progress = Arc::new(AtomicUsize::new(0));
    let armed = Arc::new(AtomicBool::new(false));
    let progress_probe = Arc::clone(&progress);
    let armed_probe = Arc::clone(&armed);
    let arming_thread = thread::spawn({
        let armed = Arc::clone(&armed);
        move || {
            std::thread::sleep(Duration::from_millis(20));
            armed.store(true, Ordering::SeqCst);
        }
    });
    let worker = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let probe = tokio::spawn(async move {
                while !armed_probe.load(Ordering::SeqCst) {
                    tokio::task::yield_now().await;
                }
                progress_probe.fetch_add(1, Ordering::SeqCst);
            });

            let result =
                backend.submit_outbound_http_to_blocking_pool("billing", 64, &execution_context());
            assert!(result.is_ok(), "unexpected error: {result:?}");
            probe.await.unwrap();
        });
    });

    std::thread::sleep(Duration::from_millis(50));
    assert!(
        progress.load(Ordering::SeqCst) > 0,
        "worker thread was pinned while outbound HTTP was in flight"
    );

    worker.join().unwrap();
    arming_thread.join().unwrap();
    server.join().unwrap();
}
