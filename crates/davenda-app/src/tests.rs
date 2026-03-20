use super::*;

use std::fs;

use davenda_assets::ReleaseId;
use davenda_auth::{Capability, DefaultAuthModelPackage};
use davenda_config::PlatformConfig;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, BulkOperationKind, BulkOperationScope,
    CapabilityContract, ReportDeliveryMode, ReportFormat, ReportSensitivity, SearchDocumentKind,
    SearchFieldContribution, SearchFieldRole, SearchIndexContribution, SearchInvalidationRule,
    SearchInvalidationTrigger, SearchRebuildStrategy, SearchVisibility,
};
use davenda_data::{
    MigrationId, MigrationOwner as DataMigrationOwner, MigrationPlan, MigrationStep,
};
use davenda_wasm::{
    ContractVersion, ExtensionArtifactSource, ExtensionConfigField, ExtensionConfigSchema,
    ExtensionConfigValue, ExtensionConfigValueType, ExtensionManifest, ExtensionPackage,
    ExtensionPoint, HandlerId, HandlerInstallation, HandlerManifest, HostCapabilityGrant,
    HostGrantSet, RenderHookExtensionPoint, ResourceLimits,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

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

fn set_object_store_secret(endpoint: &str) -> EnvVarGuard {
    let previous = std::env::var_os("OBJECT_STORE_URL");
    unsafe {
        std::env::set_var(
            "OBJECT_STORE_URL",
            format!(
                "endpoint_url = \"{endpoint}\"\n\
bucket = \"runtime\"\n\
region = \"us-east-1\"\n\
access_key_id = \"runtime-access\"\n\
secret_access_key = \"runtime-secret\"\n"
            ),
        );
    }

    EnvVarGuard {
        key: "OBJECT_STORE_URL",
        previous,
    }
}

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
        let handle = thread::spawn(move || {
            loop {
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
            }
        });
        thread::sleep(Duration::from_millis(25));

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
    let path = parts
        .next()
        .unwrap_or("/")
        .trim_start_matches('/')
        .to_string();

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

    let etag_header = if method == "PUT" {
        "ETag: \"test-etag\"\r\n"
    } else {
        ""
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{etag_header}Connection: close\r\n\r\n",
        response_body.len()
    );
    stream.write_all(response.as_bytes()).unwrap();
    if !response_body.is_empty() {
        stream.write_all(&response_body).unwrap();
    }
}

fn locale(value: &str) -> LocaleTag {
    LocaleTag::new(value).expect("locale is valid")
}

fn theme() -> ThemeProfile {
    ThemeProfile::new(
        ThemeId::new("harbor").unwrap(),
        vec![
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateNamespace::new("harbor").unwrap(),
        ],
    )
    .unwrap()
    .with_asset_root("theme/assets")
    .unwrap()
}

fn theme_workspace() -> TempDir {
    let workspace = tempfile::tempdir().unwrap();
    let theme_root = workspace.path().join("theme/assets");
    fs::create_dir_all(&theme_root).unwrap();
    fs::write(theme_root.join("site.css"), b"body { color: #111; }").unwrap();
    fs::write(theme_root.join("logo.svg"), b"<svg viewBox=\"0 0 1 1\" />").unwrap();
    workspace
}

fn auth() -> AuthStrategy {
    AuthStrategy::new(
        AuthMode::Extend,
        DefaultAuthModelPackage::default().manifest().name.clone(),
    )
    .unwrap()
}

fn app() -> CustomerAppManifest {
    CustomerAppManifest::new(
        CustomerAppId::new("harbor-shop").unwrap(),
        "Harbor Shop",
        locale("en-GB"),
        vec![locale("en-GB"), locale("fr-FR")],
        theme(),
        auth(),
    )
    .unwrap()
    .with_domain(AppDomain::new("shop.example.com", true).unwrap())
    .with_module(InstalledModuleSpec::new("cms").unwrap())
    .with_module(InstalledModuleSpec::new("commerce").unwrap())
    .with_content_model(
        ContentModel::new(
            "landing_page",
            "page",
            vec![
                ContentField::new("title", ContentFieldType::Text)
                    .unwrap()
                    .localized()
                    .required(),
                ContentField::new("hero_image", ContentFieldType::Asset).unwrap(),
            ],
        )
        .unwrap(),
    )
    .with_customer_migration(MigrationContract::new(
        "customer.content",
        90,
        "Creates customer app landing-page projections",
    ))
    .with_extension(
        CustomerExtension::new(
            "loyalty-widget",
            ContractVersion::new(1, 2, 3),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            ExtensionInstallation::new(
                "harbor-shop",
                vec![HandlerInstallation::new(
                    HandlerId::new("account.loyalty.widget").unwrap(),
                    HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                        slot: "cms.page.render".to_string(),
                    }]),
                )],
            )
            .unwrap(),
        )
        .unwrap()
        .with_config_value(
            "program_slug",
            ExtensionConfigValue::String("harbor-club".to_string()),
        )
        .unwrap(),
    )
}

#[test]
fn theme_profile_builds_a_publication_plan_from_typed_asset_roots() {
    let root = tempfile::tempdir().unwrap();
    let theme_root = root.path().join("theme/assets");
    fs::create_dir_all(&theme_root).unwrap();
    fs::write(theme_root.join("site.css"), b"body { color: #111; }").unwrap();

    let profile = theme();
    assert_eq!(profile.asset_roots()[0].source_root(), "theme/assets");

    let plan = profile
        .publication_plan(ReleaseId::new("release-theme-assets").unwrap(), root.path())
        .unwrap();

    assert_eq!(plan.release().release_id().as_str(), "release-theme-assets");
    assert_eq!(plan.release().artifacts().len(), 1);
}

fn extension_package() -> ExtensionPackage {
    ExtensionPackage::new(
        "worka",
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("loyalty-widget").unwrap(),
            "Loyalty Widget",
            ContractVersion::new(1, 2, 3),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(davenda_wasm::ExtensionPointKind::RenderHook),
            vec![
                HandlerManifest::new(
                    HandlerId::new("account.loyalty.widget").unwrap(),
                    "exports.loyalty_widget",
                    ExtensionPoint::RenderHook(
                        RenderHookExtensionPoint::new("cms.page.render").unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                        slot: "cms.page.render".to_string(),
                    }]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionArtifactSource::local_path("extensions/loyalty-widget.wasm").unwrap(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ExtensionConfigSchema::new(
            1,
            vec![
                ExtensionConfigField::required("program_slug", ExtensionConfigValueType::String)
                    .unwrap(),
                ExtensionConfigField::optional("show_points", ExtensionConfigValueType::Boolean)
                    .unwrap()
                    .with_default(ExtensionConfigValue::Boolean(true))
                    .unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap()
}

fn module_manifests() -> Vec<ModuleManifest> {
    vec![
        ModuleManifest::new("cms")
            .with_optional_capabilities(vec![Capability::CmsPagePublish])
            .with_capability_contracts(vec![CapabilityContract::optional(
                Capability::CmsPagePublish,
                ["page"],
            )])
            .with_core_service_dependencies(vec![CoreServiceDependency::Seo])
            .with_migrations(vec![MigrationContract::new(
                "cms.pages",
                10,
                "Creates CMS page storage",
            )])
            .with_route_surfaces(vec![RouteSurface::new(
                "cms.page",
                davenda_core::RouteSurfaceKind::FrontendPage,
                "/pages/{slug}",
            )])
            .with_jobs(vec![JobContract::new(
                "cms.publish-scheduled",
                davenda_core::JobTriggerKind::Scheduled,
                true,
                "Publishes scheduled pages",
            )])
            .with_event_subscriptions(vec![EventSubscription::new(
                "cms.page.publish-requested",
                Some("cms.publish-scheduled"),
                "Schedules future publication work",
            )])
            .with_admin_resources(vec![AdminResourceContribution::new(
                "cms.pages",
                "/admin/pages",
                "Pages",
                "Pages",
                AdminNavigationSection::Content,
                AdminContributionKind::ResourceIndex,
                Capability::CmsPagePublish,
            )])
            .with_extension_slots(vec![davenda_core::ExtensionSlotDescriptor::new(
                davenda_core::ExtensionSlotKind::RenderHook,
                "cms.page.render",
                "Allows render-hook extensions to augment CMS page rendering",
            )])
            .with_search_contributions(vec![SearchIndexContribution::new(
                "search.cms.pages",
                SearchDocumentKind::Page,
                SearchVisibility::Public,
                true,
                vec![SearchFieldContribution::new(
                    "title",
                    "title",
                    SearchFieldRole::Title,
                    true,
                    true,
                )],
                vec![SearchInvalidationRule::new(
                    SearchInvalidationTrigger::Published,
                    "page published",
                )],
                SearchRebuildStrategy::OnInvalidate,
            )]),
        ModuleManifest::new("commerce")
            .with_optional_capabilities(vec![Capability::OrderRead])
            .with_capability_contracts(vec![CapabilityContract::optional(
                Capability::OrderRead,
                ["order"],
            )])
            .with_module_dependencies(vec![davenda_core::ModuleDependency::required(
                "cms",
                "Commerce storefront installs depend on CMS navigation and content surfaces",
            )])
            .with_core_service_dependencies(vec![CoreServiceDependency::Jobs])
            .with_report_definitions(vec![ReportDefinition::new(
                "report.orders.summary",
                "Orders summary",
                Some("Operational order summary".to_string()),
                Capability::OrderRead,
                ReportFormat::Csv,
                ReportSensitivity::Restricted,
                ReportDeliveryMode::InternalOnly,
                "reports/orders",
                davenda_jobs::RetryPolicy::new(
                    3,
                    std::time::Duration::from_secs(15),
                    std::time::Duration::from_secs(300),
                )
                .unwrap(),
            )])
            .with_bulk_operations(vec![BulkOperationDefinition::new(
                "bulk.orders.export",
                "Bulk export orders",
                Some("Queues order exports".to_string()),
                Capability::OrderRead,
                BulkOperationKind::Export,
                BulkOperationScope::Commerce,
                davenda_jobs::RetryPolicy::new(
                    3,
                    std::time::Duration::from_secs(15),
                    std::time::Duration::from_secs(300),
                )
                .unwrap(),
                Some(100),
                true,
            )]),
    ]
}

#[derive(Debug)]
struct StaticModule {
    manifest: ModuleManifest,
    migration_plan: Option<MigrationPlan>,
}

impl StaticModule {
    fn new(manifest: ModuleManifest) -> Self {
        let migration_plan = match manifest.name.as_str() {
            "cms" => Some(static_migration_plan("cms", "001_pages")),
            "commerce" => Some(static_migration_plan("commerce", "001_catalog")),
            _ => None,
        };

        Self {
            manifest,
            migration_plan,
        }
    }
}

impl PlatformModule for StaticModule {
    fn manifest(&self) -> ModuleManifest {
        self.manifest.clone()
    }

    fn register(
        &self,
        _registry: &mut davenda_core::ServiceRegistry,
    ) -> Result<(), davenda_core::RegistrationError> {
        Ok(())
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        self.migration_plan.clone()
    }
}

fn static_migration_plan(module: &str, step_id: &str) -> MigrationPlan {
    let mut plan = MigrationPlan::new();
    plan.insert(
        MigrationStep::new(
            MigrationId::new(step_id).unwrap(),
            DataMigrationOwner::Module(module.to_string()),
            10,
            format!("install {module} tables"),
        )
        .unwrap()
        .with_statement("SELECT 1")
        .unwrap(),
    )
    .unwrap();
    plan
}

fn runtime_config(app_id: &str) -> PlatformConfig {
    PlatformConfig::from_toml_str(&format!(
        r#"
[app]
name = "{app_id}"
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
object_store = "s3"
object_store_secret = {{ kind = "env", var = "OBJECT_STORE_URL" }}
local_root = "/tmp/davenda-app-tests"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "shop.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 101

[modules]
enabled = ["cms", "commerce"]

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
"#
    ))
    .expect("runtime config is valid")
}

#[test]
fn manifest_requires_supported_default_locale_and_canonical_domain() {
    let invalid = CustomerAppManifest::new(
        CustomerAppId::new("invalid").unwrap(),
        "Invalid",
        locale("en-GB"),
        vec![locale("fr-FR")],
        theme(),
        auth(),
    )
    .unwrap()
    .with_domain(AppDomain::new("preview.example.com", false).unwrap());

    assert_eq!(
        invalid.validate().unwrap_err(),
        AppModelError::DefaultLocaleNotSupported {
            default_locale: "en-GB".to_string(),
        }
    );
}

#[test]
fn manifest_rejects_duplicate_modules_and_extension_app_mismatch() {
    let duplicated = app().with_module(InstalledModuleSpec::new("cms").unwrap());
    assert_eq!(
        duplicated.validate().unwrap_err(),
        AppModelError::DuplicateInstalledModule {
            module: "cms".to_string(),
        }
    );

    let mismatched = CustomerAppManifest::new(
        CustomerAppId::new("mismatch").unwrap(),
        "Mismatch",
        locale("en-GB"),
        vec![locale("en-GB")],
        theme(),
        auth(),
    )
    .unwrap()
    .with_domain(AppDomain::new("mismatch.example.com", true).unwrap())
    .with_extension(
        CustomerExtension::new(
            "widget",
            ContractVersion::new(1, 0, 0),
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ExtensionInstallation::new("other-app", Vec::new()).unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        mismatched.validate().unwrap_err(),
        AppModelError::ExtensionCustomerAppMismatch {
            extension_id: "widget".to_string(),
            extension_customer_app: "other-app".to_string(),
            app_id: "mismatch".to_string(),
        }
    );
}

#[test]
fn composition_collects_installed_module_contracts() {
    let composition = app()
        .compose(&DefaultAuthModelPackage::default(), &module_manifests())
        .unwrap();

    assert_eq!(composition.installed_modules.len(), 2);
    assert_eq!(composition.module_list().len(), 2);
    assert_eq!(composition.route_surfaces.len(), 1);
    assert_eq!(composition.jobs.len(), 1);
    assert_eq!(composition.event_subscriptions.len(), 1);
    assert_eq!(composition.admin_resources.len(), 1);
    assert_eq!(composition.search_contributions.len(), 1);
    assert_eq!(composition.report_definitions.len(), 1);
    assert_eq!(composition.bulk_operations.len(), 1);
    assert_eq!(composition.migrations.len(), 2);
    assert_eq!(composition.canonical_domain(), Some("shop.example.com"));
    assert!(
        composition
            .required_core_services
            .contains(&CoreServiceDependency::Seo)
    );
    assert!(
        composition
            .required_core_services
            .contains(&CoreServiceDependency::Jobs)
    );
    assert_eq!(
        composition.module_list()[0].id,
        ModuleId::new("cms").unwrap()
    );
    assert_eq!(
        composition.module_list()[1].module_dependencies[0].module,
        "cms".to_string()
    );
}

#[test]
fn composition_rejects_unknown_modules_and_missing_dependencies() {
    let unknown = CustomerAppManifest::new(
        CustomerAppId::new("unknown").unwrap(),
        "Unknown",
        locale("en-GB"),
        vec![locale("en-GB")],
        theme(),
        auth(),
    )
    .unwrap()
    .with_domain(AppDomain::new("unknown.example.com", true).unwrap())
    .with_module(InstalledModuleSpec::new("events").unwrap());

    assert_eq!(
        unknown
            .compose(&DefaultAuthModelPackage::default(), &module_manifests())
            .unwrap_err(),
        AppModelError::UnknownInstalledModule {
            app_id: "unknown".to_string(),
            module: "events".to_string(),
        }
    );

    let missing_dependency = CustomerAppManifest::new(
        CustomerAppId::new("dependency").unwrap(),
        "Dependency",
        locale("en-GB"),
        vec![locale("en-GB")],
        theme(),
        auth(),
    )
    .unwrap()
    .with_domain(AppDomain::new("dependency.example.com", true).unwrap())
    .with_module(InstalledModuleSpec::new("commerce").unwrap());

    assert_eq!(
        missing_dependency
            .compose(&DefaultAuthModelPackage::default(), &module_manifests())
            .unwrap_err(),
        AppModelError::MissingModuleDependency {
            module: "commerce".to_string(),
            dependency: "cms".to_string(),
        }
    );
}

#[test]
fn customer_app_can_build_a_runtime_plan_from_selected_modules() {
    let server = ObjectStoreTestServer::spawn();
    let _guard = set_object_store_secret(server.endpoint());
    let workspace = theme_workspace();
    let runtime = app()
        .build_runtime_plan_with_extensions_at(
            runtime_config("harbor-shop"),
            DefaultAuthModelPackage::default(),
            module_manifests()
                .into_iter()
                .map(StaticModule::new)
                .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                .collect(),
            vec![extension_package()],
            workspace.path(),
        )
        .unwrap();

    assert_eq!(
        runtime.composition.app_id,
        CustomerAppId::new("harbor-shop").unwrap()
    );
    assert_eq!(runtime.runtime.config.app.name, "harbor-shop");
    assert_eq!(runtime.runtime.modules.len(), 2);
    assert_eq!(runtime.migration_summary.entries().len(), 4);
    assert!(
        runtime
            .migration_summary
            .entries()
            .iter()
            .any(|entry| matches!(
                entry.owner,
                MigrationPlanOwner::AuthPackage(ref package) if package == "platform-default-auth"
            ))
    );
    assert!(
        runtime
            .migration_summary
            .entries()
            .iter()
            .any(|entry| matches!(
                entry.owner,
                MigrationPlanOwner::CustomerApp(ref app_id) if app_id == "harbor-shop"
            ))
    );
    let theme_publication = runtime
        .theme_publication
        .as_ref()
        .expect("theme assets should be published when manifest publishing is enabled");
    assert_eq!(theme_publication.manifest().entries().count(), 2);
    assert_eq!(theme_publication.writes().len(), 2);
    assert!(!runtime.release_doctor.is_compatible());
    assert!(
        runtime
            .release_doctor
            .findings
            .iter()
            .any(|finding| finding.code == "module.ops.missing")
    );
}

#[test]
fn runtime_build_requires_pinned_extension_packages() {
    let workspace = theme_workspace();
    let error = app()
        .build_runtime_plan_at(
            runtime_config("harbor-shop"),
            DefaultAuthModelPackage::default(),
            module_manifests()
                .into_iter()
                .map(StaticModule::new)
                .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                .collect(),
            workspace.path(),
        )
        .unwrap_err();

    assert_eq!(
        error,
        AppModelError::ExtensionPackagesRequired {
            app_id: "harbor-shop".to_string(),
        }
    );

    let mut wrong_checksum = extension_package();
    wrong_checksum.artifact_sha256 =
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    let workspace = theme_workspace();
    let error = app()
        .build_runtime_plan_with_extensions_at(
            runtime_config("harbor-shop"),
            DefaultAuthModelPackage::default(),
            module_manifests()
                .into_iter()
                .map(StaticModule::new)
                .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                .collect(),
            vec![wrong_checksum],
            workspace.path(),
        )
        .unwrap_err();

    assert_eq!(
        error,
        AppModelError::ExtensionArtifactChecksumMismatch {
            extension_id: "loyalty-widget".to_string(),
            configured: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            actual: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
        }
    );
}

#[test]
fn runtime_build_rejects_config_module_drift_and_unexpected_runtime_modules() {
    let mut drifted = runtime_config("harbor-shop");
    drifted.modules.enabled.push("events".to_string());

    let workspace = theme_workspace();
    assert_eq!(
        app()
            .build_runtime_plan_at(
                drifted,
                DefaultAuthModelPackage::default(),
                module_manifests()
                    .into_iter()
                    .map(StaticModule::new)
                    .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                    .collect(),
                workspace.path(),
            )
            .unwrap_err(),
        AppModelError::ConfigModulesMismatch {
            manifest_only: Vec::new(),
            configured_only: vec!["events".to_string()],
        }
    );

    let mut modules = module_manifests()
        .into_iter()
        .map(StaticModule::new)
        .map(|module| Box::new(module) as Box<dyn PlatformModule>)
        .collect::<Vec<_>>();
    modules.push(Box::new(StaticModule::new(ModuleManifest::new("media"))));

    let workspace = theme_workspace();
    assert_eq!(
        app()
            .build_runtime_plan_at(
                runtime_config("harbor-shop"),
                DefaultAuthModelPackage::default(),
                modules,
                workspace.path(),
            )
            .unwrap_err(),
        AppModelError::UnexpectedRuntimeModules {
            app_id: "harbor-shop".to_string(),
            modules: vec!["media".to_string()],
        }
    );
}

#[test]
fn release_doctor_reports_config_drift_and_unpinned_modules() {
    let manifest = app();
    let composition = manifest
        .compose(&DefaultAuthModelPackage::default(), &module_manifests())
        .unwrap();
    let report = composition.release_doctor(Some(&runtime_config("harbor-shop")));

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "module.version.unpinned")
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "module.ops.missing")
    );

    let mut drifted = runtime_config("harbor-shop");
    drifted.seo.canonical_host = "preview.example.com".to_string();
    let report = composition.release_doctor(Some(&drifted));
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "config.seo.canonical_host")
    );

    let mut wrong_checksum = extension_package();
    wrong_checksum.artifact_sha256 =
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    let report = manifest
        .release_doctor_with_extensions(
            &DefaultAuthModelPackage::default(),
            &module_manifests(),
            &[wrong_checksum],
            Some(&runtime_config("harbor-shop")),
        )
        .unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "extension.checksum.mismatch")
    );
}

#[test]
fn customer_app_reports_render_into_cli_surfaces() {
    let server = ObjectStoreTestServer::spawn();
    let _guard = set_object_store_secret(server.endpoint());
    let workspace = theme_workspace();
    let runtime = app()
        .build_runtime_plan_with_extensions_at(
            runtime_config("harbor-shop"),
            DefaultAuthModelPackage::default(),
            module_manifests()
                .into_iter()
                .map(StaticModule::new)
                .map(|module| Box::new(module) as Box<dyn PlatformModule>)
                .collect(),
            vec![extension_package()],
            workspace.path(),
        )
        .unwrap();

    let modules = runtime.composition.module_list_report().unwrap();
    assert_eq!(
        modules.command,
        vec!["module".to_string(), "list".to_string()]
    );
    assert_eq!(modules.rows.len(), 2);

    let migrations = runtime.migration_summary.command_report().unwrap();
    assert_eq!(
        migrations.command,
        vec!["migrate".to_string(), "plan".to_string()]
    );
    assert!(migrations.rows.len() >= 4);

    let release = runtime.release_doctor.command_report().unwrap();
    assert_eq!(
        release.command,
        vec!["release".to_string(), "doctor".to_string()]
    );
    assert!(
        release
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "module.ops.missing")
    );
}
