use davenda_all::CustomerBackendPlugin;
use davenda_customer_sdk::RegisteredHookKind;
use harbor_shop_app::HarborShopWorkspace;
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
    std::env::temp_dir().join(format!("harbor-shop-{label}-{unique}-{counter}"))
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
    let source_root = HarborShopWorkspace::default()
        .unwrap()
        .app_root()
        .to_path_buf();
    let temp_root = unique_temp_app_root("chapter96");
    fs::create_dir_all(&temp_root).unwrap();
    copy_dir_recursive(&source_root.join("auth"), &temp_root.join("auth"));
    copy_dir_recursive(
        &source_root.join("extensions"),
        &temp_root.join("extensions"),
    );
    copy_dir_recursive(&source_root.join("templates"), &temp_root.join("templates"));
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
fn linked_customer_backend_descriptor_stays_stable() {
    let descriptor = harbor_shop_backend::plugin().descriptor();

    assert_eq!(descriptor.id, "harbor-shop-backend");
    assert_eq!(descriptor.display_name, "Harbor Shop Linked Backend");
}

#[test]
fn admin_dashboard_surfaces_the_linked_workspace_backend() {
    let dashboard = include_str!("../../../templates/admin/dashboard.html");
    let app_readme = include_str!("../../../README.md");
    let cargo_toml = include_str!("../../../Cargo.toml");
    let entrypoint = include_str!("../../../docker/entrypoint.sh");
    let dockerfile = include_str!("../../../Dockerfile");
    let dockerfile_repo = include_str!("../../../Dockerfile.repo");

    assert!(dashboard.contains("Linked customer backend"), "{dashboard}");
    assert!(dashboard.contains("linkedCustomerPlugins"), "{dashboard}");
    assert!(
        dashboard.contains("Workspace-owned Rust hook path"),
        "{dashboard}"
    );
    assert!(
        dashboard.contains("cargo run -p harbor-shop -- linked-backend demo"),
        "{dashboard}"
    );

    assert!(
        app_readme.contains("cargo run -p harbor-shop -- describe"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("cargo run -p harbor-shop -- linked-backend demo"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("cargo run -p harbor-shop -- validate"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("cargo run -p harbor-shop -- migrate apply --dry-run"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("cargo run -p harbor-shop -- assets publish"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("cargo run -p harbor-shop -- up"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("./scripts/prepare-local-dev.sh"),
        "{app_readme}"
    );
    assert!(
        app_readme
            .contains("docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("uses only the Harbor Shop folder as its Docker build context"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("free of `patch.crates-io` overlays"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("writes `apps/harbor-shop/.cargo/config.toml`"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("Harbor Shop Linked Backend"),
        "{app_readme}"
    );
    assert!(!cargo_toml.contains("[patch.crates-io]"), "{cargo_toml}");
    assert!(
        cargo_toml.contains("davenda-all = \"0.1.0\""),
        "{cargo_toml}"
    );
    assert!(
        entrypoint.contains("exec harbor-shop up --config"),
        "{entrypoint}"
    );
    assert!(
        dockerfile.contains("cargo build --locked -p harbor-shop --release"),
        "{dockerfile}"
    );
    assert!(
        dockerfile.contains("COPY --from=builder /workspace/harbor-shop /usr/local/bin/harbor-shop"),
        "{dockerfile}"
    );
    assert!(!dockerfile.contains("davenda-cli"), "{dockerfile}");
    assert!(
        dockerfile_repo.contains("WORKDIR /workspace/apps/harbor-shop"),
        "{dockerfile_repo}"
    );
    assert!(
        dockerfile_repo.contains("cargo build --locked -p harbor-shop --release"),
        "{dockerfile_repo}"
    );
    assert!(!dockerfile_repo.contains("davenda-cli"), "{dockerfile_repo}");
}

#[test]
fn workspace_summary_reports_real_linked_plugin_details() {
    let workspace = HarborShopWorkspace::default().unwrap();
    let summary = workspace.describe("platform.dev.toml").unwrap();

    assert_eq!(summary.linked_plugin_ids, vec!["harbor-shop-backend"]);
    assert_eq!(summary.linked_plugins.len(), 1);
    let plugin = &summary.linked_plugins[0];
    assert_eq!(plugin.id, "harbor-shop-backend");
    assert_eq!(plugin.display_name, "Harbor Shop Linked Backend");
    assert_eq!(
        plugin.documentation_url.as_deref(),
        Some("apps/harbor-shop/backend/README.md")
    );
    assert_eq!(
        plugin.hook_kinds,
        vec![
            RegisteredHookKind::Checkout,
            RegisteredHookKind::VerifiedWebhook,
        ]
    );
}

#[test]
fn workspace_bootstrap_registers_the_linked_backend_into_the_runtime_plan() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _object_store = set_env_var(
        "OBJECT_STORE_URL",
        r#"
endpoint_url = "https://s3.internal"
bucket = "runtime"
region = "eu-west-2"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
signed_url_ttl_secs = 900
"#,
    );
    let temp_root = temp_workspace_without_theme_assets();
    let workspace = HarborShopWorkspace::at(&temp_root.path).unwrap();
    let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();

    assert_eq!(bootstrap.linked_plugin_ids(), vec!["harbor-shop-backend"]);
    assert_eq!(
        bootstrap.runtime_plan.runtime.linked_customer_plugins.len(),
        1
    );
    let plugin = &bootstrap.runtime_plan.runtime.linked_customer_plugins[0];
    assert_eq!(plugin.plugin_id, "harbor-shop-backend");
    assert_eq!(plugin.display_name, "Harbor Shop Linked Backend");
    assert_eq!(
        plugin.registered_hooks,
        vec![
            RegisteredHookKind::Checkout,
            RegisteredHookKind::VerifiedWebhook,
        ]
    );
}

#[test]
fn workspace_validate_reports_customer_owned_lifecycle_summary() {
    let workspace = HarborShopWorkspace::default().unwrap();
    let validation = workspace.validate("platform.dev.toml").unwrap();

    assert_eq!(validation.app_id, "harbor-shop");
    assert!(
        validation
            .module_ids
            .iter()
            .any(|module| module == "commerce")
    );
    assert_eq!(validation.linked_plugin_ids, vec!["harbor-shop-backend"]);
    assert!(validation.route_surface_count > 0);
    assert!(validation.job_count > 0);
}

#[test]
fn workspace_publish_assets_reports_noop_without_theme_roots() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _object_store = set_env_var(
        "OBJECT_STORE_URL",
        r#"
endpoint_url = "https://s3.internal"
bucket = "runtime"
region = "eu-west-2"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
signed_url_ttl_secs = 900
"#,
    );
    let temp_root = temp_workspace_without_theme_assets();
    let workspace = HarborShopWorkspace::at(&temp_root.path).unwrap();
    let report = workspace.publish_assets("platform.dev.toml").unwrap();

    assert_eq!(report.app_id, "harbor-shop");
    assert!(!report.published);
    assert!(report.asset_roots.is_empty());
    assert_eq!(report.asset_entries, 0);
    assert_eq!(report.writes, 0);
}

#[test]
fn workspace_migrate_dry_run_reports_pending_executable_steps() {
    let _env_lock = ENV_LOCK.lock().unwrap();
    let _object_store = set_env_var(
        "OBJECT_STORE_URL",
        r#"
endpoint_url = "https://s3.internal"
bucket = "runtime"
region = "eu-west-2"
access_key_id = "runtime-access"
secret_access_key = "runtime-secret"
signed_url_ttl_secs = 900
"#,
    );
    let temp_root = temp_workspace_without_theme_assets();
    let workspace = HarborShopWorkspace::at(&temp_root.path).unwrap();
    let report = workspace.migrate_apply("platform.dev.toml", true).unwrap();

    assert!(report.dry_run);
    assert_eq!(report.app_id, "harbor-shop");
    assert_eq!(report.pending_steps, report.executable_steps);
    assert_eq!(report.already_applied_steps, 0);
    assert_eq!(report.executed_statements, 0);
}
