use axum::body::{Body, to_bytes};
use axum::http::Request;
use coil_runtime::EnvironmentSecretResolver;
use shoppr_app::{
    ShopprWorkspace, shoppr_waitlist_ops_widget_demo_sha256, shoppr_waitlist_tools_demo_sha256,
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
    std::env::temp_dir().join(format!("shoppr-{label}-{unique}-{counter}"))
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
    let source_root = ShopprWorkspace::default()
        .unwrap()
        .app_root()
        .to_path_buf();
    let temp_root = unique_temp_app_root("chapter80");
    fs::create_dir_all(&temp_root).unwrap();
    copy_dir_recursive(&source_root.join("auth"), &temp_root.join("auth"));
    copy_dir_recursive(&source_root.join("templates"), &temp_root.join("templates"));
    copy_dir_recursive(
        &source_root.join("translations"),
        &temp_root.join("translations"),
    );
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
fn harbor_readme_and_manifest_make_runtime_installed_wasm_concrete() {
    let app_readme = include_str!("../../../README.md");
    let extensions_readme = include_str!("../../../extensions/README.md");
    let waitlist_readme = include_str!("../../../extensions/shoppr-waitlist-tools/README.md");
    let package = include_str!("../../../extensions/shoppr-waitlist-tools/package.toml");
    let source_wat =
        include_str!("../../../extensions/shoppr-waitlist-tools/shoppr-waitlist-tools.wat");
    let app_manifest = include_str!("../../../app.toml");

    assert!(app_readme.contains("shoppr-waitlist-tools"), "{app_readme}");
    assert!(app_readme.contains("runtime-installed"), "{app_readme}");
    assert!(
        app_readme.contains("linked customer Rust crate"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("real installed extension entry in `app.toml`"),
        "{app_readme}"
    );

    assert!(
        extensions_readme.contains("linked Rust is the primary path"),
        "{extensions_readme}"
    );
    assert!(
        extensions_readme.contains("loaded through `package.toml`"),
        "{extensions_readme}"
    );

    assert!(
        waitlist_readme.contains("real `[[extensions]]` installation entry"),
        "{waitlist_readme}"
    );
    assert!(waitlist_readme.contains("render hook"), "{waitlist_readme}");

    assert!(
        package.contains("id = \"shoppr-waitlist-tools\""),
        "{package}"
    );
    assert!(package.contains("point = \"render-hook\""), "{package}");
    assert!(
        package.contains("target = \"cms.page.render\""),
        "{package}"
    );
    assert!(
        source_wat.contains("__COIL_HANDLER_EXPORT__"),
        "{source_wat}"
    );

    assert!(app_manifest.contains("[[extensions]]"), "{app_manifest}");
    assert!(
        app_manifest.contains("shoppr-waitlist-tools"),
        "{app_manifest}"
    );
}

#[test]
fn harbor_waitlist_tools_declares_the_real_demo_checksum() {
    let app_root = ShopprWorkspace::default()
        .unwrap()
        .app_root()
        .to_path_buf();
    let expected = shoppr_waitlist_tools_demo_sha256(&app_root).unwrap();
    let package = include_str!("../../../extensions/shoppr-waitlist-tools/package.toml");
    let app_manifest = include_str!("../../../app.toml");

    assert!(
        package.contains(&format!("artifact_sha256 = \"{expected}\"")),
        "expected checksum {expected}\n{package}"
    );
    assert!(
        app_manifest.contains(&format!("artifact_sha256 = \"{expected}\"")),
        "expected checksum {expected}\n{app_manifest}"
    );
}

#[test]
fn waitlist_ops_widget_checksum_probe() {
    let app_root = ShopprWorkspace::default()
        .unwrap()
        .app_root()
        .to_path_buf();
    let expected = shoppr_waitlist_ops_widget_demo_sha256(&app_root).unwrap();
    println!("{expected}");
    assert_eq!(expected.len(), 64);
}

#[test]
fn workspace_bootstrap_installs_the_runtime_wasm_extension() {
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
    let workspace = ShopprWorkspace::at(&temp_root.path).unwrap();
    let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();

    let manifest_extension_ids = bootstrap
        .manifest
        .extensions
        .iter()
        .map(|extension| extension.id.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        manifest_extension_ids,
        vec![
            "shoppr-waitlist-tools".to_string(),
            "shoppr-waitlist-ops-widget".to_string()
        ]
    );

    let installed_extension_ids = bootstrap
        .runtime_plan
        .runtime
        .installed_extensions
        .iter()
        .map(|extension| extension.extension_id.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        installed_extension_ids,
        vec![
            "shoppr-waitlist-tools".to_string(),
            "shoppr-waitlist-ops-widget".to_string()
        ]
    );
}

#[test]
fn harbor_home_page_executes_the_installed_wasm_render_hook() {
    let _env_lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    let _database_url = set_env_var(
        "DATABASE_URL",
        "postgres://harbor:harbor@127.0.0.1:5432/harbor_shop",
    );
    let _stripe_secret = set_env_var("STRIPE_SECRET_KEY", "sk_test_harbor_runtime");
    let _stripe_publishable = set_env_var("STRIPE_PUBLISHABLE_KEY", "pk_test_harbor_runtime");
    let _stripe_webhook_secret = set_env_var("STRIPE_WEBHOOK_SECRET", "whsec_harbor_runtime");
    let temp_root = temp_workspace_without_theme_assets();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (headers, body) = runtime.block_on(async move {
        let workspace = ShopprWorkspace::at(&temp_root.path).unwrap();
        let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();
        let resolver = EnvironmentSecretResolver::default();
        let server = bootstrap
            .runtime_plan
            .runtime
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
                    .uri("/en-GB/pages/home")
                    .header("host", "www.example.com")
                    .header("x-forwarded-proto", "https")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let headers = response.headers().clone();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        std::mem::forget(server);
        std::mem::forget(bootstrap);
        (headers, body)
    });

    assert_eq!(
        headers.get("x-coil-wasm-render-hook-count").unwrap(),
        "1"
    );
    assert_eq!(
        headers.get("x-coil-wasm-render-hook-handlers").unwrap(),
        "home.waitlist.banner"
    );
    assert!(body.contains("Shoppr Waitlist Tools is active"), "{body}");
}
