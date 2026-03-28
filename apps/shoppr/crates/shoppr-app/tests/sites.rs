use axum::body::{Body, to_bytes};
use axum::http::Request;
use davenda_runtime::EnvironmentSecretResolver;
use shoppr_app::ShopprWorkspace;
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
    let temp_root = unique_temp_app_root("sites");
    fs::create_dir_all(&temp_root).unwrap();
    copy_dir_recursive(&source_root.join("auth"), &temp_root.join("auth"));
    copy_dir_recursive(&source_root.join("templates"), &temp_root.join("templates"));
    copy_dir_recursive(&source_root.join("extensions"), &temp_root.join("extensions"));
    if source_root.join("translations").is_dir() {
        copy_dir_recursive(&source_root.join("translations"), &temp_root.join("translations"));
    }
    if source_root.join("theme").is_dir() {
        copy_dir_recursive(&source_root.join("theme"), &temp_root.join("theme"));
    }
    fs::copy(
        source_root.join("platform.dev.toml"),
        temp_root.join("platform.dev.toml"),
    )
    .unwrap();
    fs::copy(source_root.join("catalog.toml"), temp_root.join("catalog.toml")).unwrap();
    let app_manifest = fs::read_to_string(source_root.join("app.toml")).unwrap();
    let app_manifest = app_manifest.replace("asset_roots = [\"theme/assets\"]", "asset_roots = []");
    fs::write(temp_root.join("app.toml"), app_manifest).unwrap();
    TempAppRoot { path: temp_root }
}

#[test]
fn shoppr_manifest_declares_three_sites_with_distinct_market_hosts() {
    let workspace = ShopprWorkspace::default().unwrap();
    let manifest = workspace.load_manifest().unwrap();
    let sites = manifest
        .sites
        .iter()
        .map(|site| {
            (
                site.id.as_str(),
                site.canonical_domain().unwrap_or_default(),
                site.default_locale.as_str(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        sites,
        vec![
            ("shoppr-uk", "uk.localhost", "en-GB"),
            ("shoppr-fr", "fr.localhost", "fr-FR"),
            ("shoppr-pl", "pl.localhost", "pl-PL"),
        ]
    );
}

#[test]
fn shoppr_home_page_surfaces_three_market_demo_cards() {
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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

    let body = runtime.block_on(async move {
        let workspace = ShopprWorkspace::at(&temp_root.path).unwrap();
        let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();
        let resolver = EnvironmentSecretResolver::default();
        let server = bootstrap
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
                    .uri("/")
                    .header("host", "uk.localhost")
                    .header("x-forwarded-proto", "https")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        std::mem::forget(server);
        std::mem::forget(bootstrap);
        body
    });

    assert!(body.contains("One customer app, three Shoppr markets"), "{body}");
    assert!(body.contains("UK flagship storefront"), "{body}");
    assert!(body.contains("France city edit"), "{body}");
    assert!(body.contains("Poland cold-weather edit"), "{body}");
    assert!(body.contains("/en-GB/shop"), "{body}");
    assert!(body.contains("/fr-FR/events"), "{body}");
    assert!(body.contains("/pl-PL/shop/products/harbor-scarf"), "{body}");
}

#[test]
fn shoppr_home_page_uses_server_side_translation_catalogs_for_market_defaults_and_locale_paths() {
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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

    let (fr_body, pl_body) = runtime.block_on(async move {
        let workspace = ShopprWorkspace::at(&temp_root.path).unwrap();
        let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();
        let resolver = EnvironmentSecretResolver::default();
        let server = bootstrap
            .server_host(
                &resolver,
                b"01234567012345670123456701234567",
                b"76543210765432107654321076543210",
            )
            .unwrap();

        let fr_response = server
            .respond(
                Request::builder()
                    .method("GET")
                    .uri("/")
                    .header("host", "fr.localhost")
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

        let pl_response = server
            .respond(
                Request::builder()
                    .method("GET")
                    .uri("/pl-PL")
                    .header("host", "uk.localhost")
                    .header("x-forwarded-proto", "https")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let pl_body = String::from_utf8(
            to_bytes(pl_response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();

        std::mem::forget(server);
        std::mem::forget(bootstrap);
        (fr_body, pl_body)
    });

    assert!(fr_body.contains("Une application cliente, trois marchés Shoppr."), "{fr_body}");
    assert!(fr_body.contains("Voir les nouveautés"), "{fr_body}");
    assert!(fr_body.contains("Marché"), "{fr_body}");
    assert!(pl_body.contains("Jedna aplikacja klienta, trzy rynki Shoppr."), "{pl_body}");
    assert!(pl_body.contains("Zobacz nowości"), "{pl_body}");
    assert!(pl_body.contains("Rynek"), "{pl_body}");
}

#[test]
fn shoppr_three_sites_map_to_distinct_market_routes() {
    let _env_lock = ENV_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
    let workspace = ShopprWorkspace::at(&temp_root.path).unwrap();
    let bootstrap = workspace.build_bootstrap("platform.dev.toml").unwrap();
    let catalog = &bootstrap.runtime_plan.runtime.storefront_catalog;

    assert!(
        catalog
            .visible_collection_for_site(Some("shoppr-uk"), "memberships")
            .is_some()
    );
    assert!(
        catalog
            .visible_collection_for_site(Some("shoppr-fr"), "memberships")
            .is_none()
    );
    assert!(
        catalog
            .visible_collection_for_site(Some("shoppr-fr"), "events")
            .is_some()
    );
    assert!(
        catalog
            .visible_product_for_site(Some("shoppr-fr"), "brooklyn-night-pass")
            .is_some()
    );
    assert!(
        catalog
            .visible_product_for_site(Some("shoppr-uk"), "brooklyn-night-pass")
            .is_none()
    );
    assert!(
        catalog
            .visible_product_for_site(Some("shoppr-pl"), "harbor-scarf")
            .is_some()
    );
    assert!(
        catalog
            .visible_product_for_site(Some("shoppr-fr"), "harbor-scarf")
            .is_none()
    );
    assert!(
        catalog
            .product_by_sku_or_handle_for_site(Some("shoppr-uk"), "membership-gold")
            .is_some()
    );
    assert!(
        catalog
            .product_by_sku_or_handle_for_site(Some("shoppr-fr"), "membership-gold")
            .is_none()
    );
}
