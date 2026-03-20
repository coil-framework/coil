use super::*;
use crate::{PathPolicyRule, StoragePlanRequest, StoragePlanner, StoragePolicy, StoragePolicySet};
use davenda_config::{PlatformConfig, StorageClass};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

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
        let handle = thread::spawn(move || loop {
            if stop_thread.load(Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    let store = Arc::clone(&store_thread);
                    handle_request(stream, &store);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("object-store test server failed: {error}"),
            }
        });

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

fn handle_request(mut stream: std::net::TcpStream, store: &Arc<Mutex<BTreeMap<String, Vec<u8>>>>) {
    stream.set_nonblocking(false).unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    reader.read_line(&mut request_line).unwrap();
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/").trim_start_matches('/').to_string();

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

    let (status, response_body) = match method {
        "PUT" => {
            store.lock().unwrap().insert(path, body);
            ("200 OK", Vec::new())
        }
        "GET" => match store.lock().unwrap().get(&path).cloned() {
            Some(bytes) => ("200 OK", bytes),
            None => ("404 Not Found", b"not found".to_vec()),
        },
        _ => ("405 Method Not Allowed", b"method not allowed".to_vec()),
    };

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response_body.len()
    );
    stream.write_all(response.as_bytes()).unwrap();
    if !response_body.is_empty() {
        stream.write_all(&response_body).unwrap();
    }
}

fn test_config() -> PlatformConfig {
    PlatformConfig::from_toml_str(
        r#"
[app]
name = "davenda-storage-tests"
environment = "development"

[server]
bind = "127.0.0.1:3000"
trusted_proxies = []

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

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
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
local_root = "/tmp/davenda-storage-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en"
supported_locales = ["en"]
fallback_locale = "en"
localized_routes = false

[seo]
canonical_host = "example.test"
emit_json_ld = false

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms"]

[wasm]
directory = "/tmp/davenda-storage-tests"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#,
    )
    .unwrap()
}

fn planner() -> StoragePlanner {
    StoragePlanner::new(
        crate::StorageTopology::from_config(&test_config()),
        StoragePolicySet::default().with_rule(
            PathPolicyRule::new(
                "uploads/marketing",
                Some(StorageClass::PublicUpload),
                StoragePolicy::public_upload(),
            )
            .unwrap()
            .with_object_prefix("public/marketing")
            .unwrap(),
        ),
    )
}

#[test]
fn object_store_execution_writes_reads_and_resolves_delivery_locations() {
    let server = ObjectStoreTestServer::spawn();
    let planner = planner();
    let executor = StorageExecutor::from_topology_and_object_store(
        planner.topology(),
        Some(ObjectStoreClientConfig::new(server.endpoint())),
    );
    let public_plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("uploads/marketing/hero.webp")
                .with_storage_class(StorageClass::PublicUpload),
        )
        .unwrap();

    let receipt = executor.execute_write(&public_plan, b"hero-bytes").unwrap();
    let object_key = public_plan.object_key.as_deref().unwrap();
    assert_eq!(receipt.target.backend, StorageBackendKind::S3Compatible);
    assert!(receipt.path.ends_with(object_key));
    assert_eq!(
        executor.execute_read(&public_plan).unwrap().bytes,
        b"hero-bytes"
    );
    assert_eq!(
        executor
            .delivery_location(&public_plan, Some("https://cdn.example.com"))
            .unwrap(),
        StorageDeliveryLocation::PublicCdn {
            public_url: format!("https://cdn.example.com/{}", object_key),
            object_key: object_key.to_string(),
        }
    );

    let private_plan = planner
        .plan_scalable_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(StorageClass::PrivateShared),
        )
        .unwrap();
    let private_object_key = private_plan.object_key.as_deref().unwrap();
    assert_eq!(
        executor.delivery_location(&private_plan, None).unwrap(),
        StorageDeliveryLocation::SignedObject {
            object_key: private_object_key.to_string(),
        }
    );
}

#[test]
fn local_disk_execution_writes_reads_and_resolves_local_delivery() {
    let mut config = test_config();
    config.storage.object_store = None;
    let planner = StoragePlanner::from_config(&config);
    let executor = StorageExecutor::from_topology(planner.topology());
    let plan = planner
        .single_node_escape_hatch()
        .plan_write(
            StoragePlanRequest::new("secure/reports/march.csv")
                .with_storage_class(StorageClass::PrivateShared)
                .with_override(crate::StoragePolicyOverride::force_single_node_escape_hatch()),
        )
        .unwrap();

    let receipt = executor.execute_write(&plan, b"sensitive-bytes").unwrap();
    assert_eq!(receipt.target.backend, StorageBackendKind::LocalDisk);
    assert_eq!(receipt.bytes_written, "sensitive-bytes".len() as u64);
    assert_eq!(
        receipt.path,
        PathBuf::from("/tmp/davenda-storage-tests/secure/reports/march.csv")
    );
    assert_eq!(
        executor.execute_read(&plan).unwrap().bytes,
        b"sensitive-bytes"
    );
    assert_eq!(
        executor.delivery_location(&plan, None).unwrap(),
        StorageDeliveryLocation::LocalPath {
            path: PathBuf::from("/tmp/davenda-storage-tests/secure/reports/march.csv"),
        }
    );
}
