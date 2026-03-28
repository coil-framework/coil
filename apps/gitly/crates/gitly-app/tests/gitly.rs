use axum::body::{Body, to_bytes};
use axum::http::Request;
use davenda_runtime::EnvironmentSecretResolver;
use gitly_app::{
    GitlyWorkspace, gitly_actions_scheduler_demo_sha256, gitly_community_pulse_demo_sha256,
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    LazyLock, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

struct EnvVarGuard {
    key: &'static str,
    previous: Option<OsString>,
}

struct RuntimeEnvGuards {
    _object_store: EnvVarGuard,
    _database_url: EnvVarGuard,
    _redis_url: EnvVarGuard,
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

fn set_env_var(key: &'static str, value: &str) -> EnvVarGuard {
    let previous = std::env::var_os(key);
    unsafe {
        std::env::set_var(key, value);
    }
    EnvVarGuard { key, previous }
}

fn set_runtime_env_vars() -> RuntimeEnvGuards {
    RuntimeEnvGuards {
        _object_store: set_env_var(
            "OBJECT_STORE_URL",
            r#"
endpoint_url = "https://s3.internal"
bucket = "runtime"
region = "eu-west-2"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
signed_url_ttl_secs = 900
"#,
        ),
        _database_url: set_env_var(
            "DATABASE_URL",
            "postgres://gitly:gitly@127.0.0.1:5432/gitly",
        ),
        _redis_url: set_env_var("REDIS_URL", "redis://127.0.0.1:6379"),
    }
}

struct TempAppRoot {
    path: PathBuf,
}

impl Drop for TempAppRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_temp_app_root(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("gitly-{label}-{unique}-{counter}"))
}

fn copy_dir_recursive(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir_recursive(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn temp_workspace_without_theme_assets() -> TempAppRoot {
    let source_root = GitlyWorkspace::default().unwrap().app_root().to_path_buf();
    let temp_root = unique_temp_app_root("tests");
    fs::create_dir_all(&temp_root).unwrap();
    copy_dir_recursive(&source_root.join("auth"), &temp_root.join("auth"));
    copy_dir_recursive(&source_root.join("templates"), &temp_root.join("templates"));
    copy_dir_recursive(
        &source_root.join("extensions"),
        &temp_root.join("extensions"),
    );
    if source_root.join("theme").is_dir() {
        copy_dir_recursive(&source_root.join("theme"), &temp_root.join("theme"));
    }
    fs::copy(
        source_root.join("platform.dev.toml"),
        temp_root.join("platform.dev.toml"),
    )
    .unwrap();
    let app_manifest = fs::read_to_string(source_root.join("app.toml")).unwrap();
    let app_manifest = app_manifest.replace("asset_roots = [\"theme/assets\"]", "asset_roots = []");
    fs::write(temp_root.join("app.toml"), app_manifest).unwrap();
    TempAppRoot { path: temp_root }
}

#[test]
fn manifest_declares_gitly_showcase_module_and_multilingual_support() {
    let workspace = GitlyWorkspace::default().unwrap();
    let manifest = workspace.load_manifest().unwrap();

    assert_eq!(manifest.id.as_str(), "gitly");
    assert_eq!(manifest.default_locale.as_str(), "en-GB");
    assert_eq!(
        manifest
            .supported_locales
            .iter()
            .map(|locale| locale.as_str())
            .collect::<Vec<_>>(),
        vec!["en-GB", "fr-FR", "de-DE"]
    );
    assert!(manifest.localized_routes);
    assert!(
        manifest
            .modules
            .iter()
            .any(|module| module.id.as_str() == "gitly-showcase")
    );
}

#[test]
fn extension_package_hash_helpers_match_the_checked_in_manifests() {
    let workspace = GitlyWorkspace::default().unwrap();
    let pulse = gitly_community_pulse_demo_sha256(workspace.app_root()).unwrap();
    let scheduler = gitly_actions_scheduler_demo_sha256(workspace.app_root()).unwrap();
    let app_manifest = include_str!("../../../app.toml");
    let pulse_package = include_str!("../../../extensions/gitly-community-pulse/package.toml");
    let scheduler_package =
        include_str!("../../../extensions/gitly-actions-scheduler/package.toml");

    assert!(app_manifest.contains(&pulse), "{app_manifest}");
    assert!(app_manifest.contains(&scheduler), "{app_manifest}");
    assert!(pulse_package.contains(&pulse), "{pulse_package}");
    assert!(
        scheduler_package.contains(&scheduler),
        "{scheduler_package}"
    );
}

#[test]
fn bootstrap_registers_linked_backend_extensions_and_mock_actions_job() {
    let _env_lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _runtime_env = set_runtime_env_vars();
    let temp_root = temp_workspace_without_theme_assets();
    let workspace = GitlyWorkspace::at(&temp_root.path).unwrap();
    let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();

    assert!(
        bootstrap
            .linked_plugin_ids()
            .contains(&"gitly-backend".to_string())
    );
    assert_eq!(bootstrap.runtime_plan.runtime.installed_extensions.len(), 2);
    assert!(
        bootstrap
            .runtime_plan
            .runtime
            .registered_runtime_jobs
            .iter()
            .any(|job| job.contract.name == "github.actions.refresh")
    );
    assert!(
        bootstrap
            .runtime_plan
            .runtime
            .http
            .routes
            .iter()
            .any(|route| route.path == "/api/github/repository")
    );
    assert!(
        bootstrap
            .runtime_plan
            .runtime
            .http
            .routes
            .iter()
            .any(|route| route.path == "/fr/octocorp/platform-ui")
    );
}

#[test]
fn server_serves_gitly_home_and_wasm_extended_api_surface() {
    let _env_lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _runtime_env = set_runtime_env_vars();
    let temp_root = temp_workspace_without_theme_assets();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let (home_body, localhost_body, api_body, fr_body, de_body, issues_body, search_body) = runtime
        .block_on(async move {
            let workspace = GitlyWorkspace::at(&temp_root.path).unwrap();
            let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();
            let resolver = EnvironmentSecretResolver::default();
            let server = bootstrap
                .server_host(
                    &resolver,
                    b"01234567012345670123456701234567",
                    b"76543210765432107654321076543210",
                )
                .unwrap();

            let home_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/")
                        .header("host", "gitly.localhost")
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

            let localhost_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/")
                        .header("host", "gitly.localhost:58080")
                        .header("x-forwarded-proto", "https")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let localhost_body = String::from_utf8(
                to_bytes(localhost_response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            let api_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/api/github/pulse")
                        .header("host", "gitly.localhost")
                        .header("x-forwarded-proto", "https")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let api_body = String::from_utf8(
                to_bytes(api_response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            let fr_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/fr")
                        .header("host", "gitly.localhost")
                        .header("x-forwarded-proto", "https")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let fr_body = String::from_utf8(
                to_bytes(fr_response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            let de_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/de")
                        .header("host", "gitly.localhost")
                        .header("x-forwarded-proto", "https")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let de_body = String::from_utf8(
                to_bytes(de_response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            let issues_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/octocorp/platform-ui/issues")
                        .header("host", "gitly.localhost")
                        .header("x-forwarded-proto", "https")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let issues_body = String::from_utf8(
                to_bytes(issues_response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            let search_response: axum::http::Response<Body> = server
                .respond(
                    Request::builder()
                        .method("GET")
                        .uri("/search?q=platform")
                        .header("host", "gitly.localhost")
                        .header("x-forwarded-proto", "https")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let search_body = String::from_utf8(
                to_bytes(search_response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

            std::mem::forget(server);
            std::mem::forget(bootstrap);
            (
                home_body,
                localhost_body,
                api_body,
                fr_body,
                de_body,
                issues_body,
                search_body,
            )
        });

    assert!(
        home_body.contains("One Davenda app can look like a forge"),
        "{home_body}"
    );
    assert!(home_body.contains("data-route=\"home\""), "{home_body}");
    assert!(home_body.contains("/octocorp/platform-ui"), "{home_body}");
    assert!(
        localhost_body.contains("data-route=\"home\""),
        "{localhost_body}"
    );
    assert!(api_body.contains("\"status\":\"active\""), "{api_body}");
    assert!(api_body.contains("\"extension\":\"ok\""), "{api_body}");
    assert!(fr_body.contains("data-route=\"home\""), "{fr_body}");
    assert!(fr_body.contains("href=\"/fr\""), "{fr_body}");
    assert!(de_body.contains("data-route=\"home\""), "{de_body}");
    assert!(de_body.contains("href=\"/de\""), "{de_body}");
    assert!(
        issues_body.contains("data-route=\"issues\""),
        "{issues_body}"
    );
    assert!(
        issues_body.contains("#402 Add keyboard shortcuts"),
        "{issues_body}"
    );
    assert!(
        search_body.contains("data-route=\"search\""),
        "{search_body}"
    );
    assert!(
        search_body.contains("data-search-results=\"results\""),
        "{search_body}"
    );
}
