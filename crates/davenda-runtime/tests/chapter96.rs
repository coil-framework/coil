use davenda_auth::DefaultAuthModelPackage;
use davenda_config::PlatformConfig;
use davenda_runtime::{RuntimeBootstrapError, RuntimeBuildError, customer_root_runtime};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const VALID_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

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
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "explicit_single_node"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/tmp/davenda-runtime-tests"
deployment = "single_node"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms"]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
"#;

struct TempAppRoot {
    path: PathBuf,
}

impl Drop for TempAppRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn unique_temp_app_root(label: &str) -> TempAppRoot {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("davenda-runtime-{label}-{unique}"));
    fs::create_dir_all(&path).unwrap();
    TempAppRoot { path }
}

fn write_template_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn write_customer_root_manifest(root: &Path, enabled_modules: &[&str]) {
    let enabled = enabled_modules
        .iter()
        .map(|module| format!("\"{module}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join("app.toml"),
        format!(
            r#"[app]
name = "showcase-events"
display_name = "Showcase Events"

[domains]
canonical = "www.example.com"
additional = []

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
localized_routes = true

[theme]
active = "showcase"
template_namespaces = ["customer-app"]
asset_roots = []

[auth]
mode = "extend"
package = "platform-default-auth"

[modules]
enabled = [{enabled}]
"#
        ),
    )
    .unwrap();
}

#[test]
fn chapter96_customer_root_builder_requires_customer_root_context() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();

    let error = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .register_module(davenda_cms::CmsModule::new())
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeBuildError::CustomerRootNotConfigured
    ));
}

#[test]
fn chapter96_customer_root_builder_filters_modules_to_manifest_enabled_set() {
    let app_root = unique_temp_app_root("chapter96-filter-modules");
    write_template_file(
        &app_root.path,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>chapter96</main></body>
</html>"#,
    );
    write_customer_root_manifest(&app_root.path, &["cms"]);
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();

    let plan = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .with_customer_root(&app_root.path)
        .register_module(davenda_cms::CmsModule::new())
        .register_module(davenda_admin::AdminModule::new())
        .build()
        .unwrap();

    assert_eq!(
        plan.modules
            .iter()
            .map(|module| module.name.as_str())
            .collect::<Vec<_>>(),
        vec!["cms"]
    );
}

#[test]
fn chapter96_customer_root_run_from_env_honors_manifest_module_filtering() {
    let app_root = unique_temp_app_root("chapter96-run-from-env");
    write_template_file(
        &app_root.path,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>chapter96</main></body>
</html>"#,
    );
    write_customer_root_manifest(&app_root.path, &["admin"]);
    let config = PlatformConfig::from_toml_str(
        &VALID_CONFIG.replace("enabled = [\"cms\"]", "enabled = [\"admin\"]"),
    )
    .unwrap();

    let error = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .with_customer_root(&app_root.path)
        .register_module(davenda_cms::CmsModule::new())
        .run_from_env()
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeBootstrapError::Build(RuntimeBuildError::CustomerManifestMissingLinkedModules {
            modules
        }) if modules == vec!["admin".to_string()]
    ));
}
