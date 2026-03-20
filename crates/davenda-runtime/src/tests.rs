use super::*;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use davenda_admin::AdminModule;
use davenda_assets::{
    AssetDeliveryTarget, AssetId, ContentFingerprint, DeliveryAudience, DeploymentArtifact,
    DeploymentRelease, FingerprintAlgorithm, ManagedAsset, ReleaseId, RevisionId,
};
use davenda_auth::{
    AuthModelManifest, AuthModelPackage, Capability, CapabilityBinding, DefaultAuthModelPackage,
    PackageMode, default_capability_bindings, default_schema,
};
use davenda_cache::{
    CacheBackendAdapter, CacheBackendKind, CacheLookupState, DistributedCacheBackend,
    InvalidationTag,
};
use davenda_cms::CmsModule;
use davenda_commerce::CommerceModule;
use davenda_config::{PlatformConfig, StorageClass, StorageDeployment};
use davenda_core::CookieSigner;
use davenda_core::{ExtensionSlotDescriptor, ExtensionSlotKind};
use davenda_events::EventsModule;
use davenda_media::MediaModule;
use davenda_memberships::MembershipsModule;
use davenda_observability::{
    CustomerAppId as FlagCustomerAppId, FeatureFlag, MaintenanceAudience, MaintenanceImpact,
};
use davenda_ops::{
    BulkExecutionId, BulkOperationId, BulkOperationRequest, OpsModule, ReportExportId,
    ReportExportRequest, ReportId,
};
use davenda_storage::{
    DeliveryMode, PathPolicyRule, StorageDeploymentScope, StoragePlanRequest, StoragePolicy,
};
use davenda_template::{
    AttributeNode, ElementNode, Node, TemplateDefinition, TemplateModelError, TemplateName,
    TemplateNamespace,
};
use davenda_tls::{
    CertificateFingerprint, CertificateId, CertificateProviderKind, CertificateRecord,
    CertificateStateStore, CertificateStatus, CloudflareEncryptionMode, CustomerAppId, Hostname,
    HostnameBinding, SecretMaterialRef, TlsInstant,
};
use davenda_wasm::{
    AdminWidgetExtensionPoint, ApiExtensionPoint, CacheVisibility, ContractVersion,
    ExtensionInstallation, ExtensionManifest, ExtensionPoint, ExtensionPointKind, HandlerId,
    HandlerInstallation, HandlerManifest, HostCall, HostCapabilityGrant, HostGrantSet,
    InstalledExtension, InvocationInput, InvocationOutcome, JobExtensionPoint, PageExtensionPoint,
    PrincipalKind, RenderHookExtensionPoint, ResourceLimits, ScheduledJobExtensionPoint,
    TypedCacheHint, TypedExecutionOutput, TypedMetadata, WasmModelError, WebhookExtensionPoint,
};
use tower::util::ServiceExt;

struct PermissiveLiveRouteCapabilityAuthorizer;

#[cfg(test)]
impl LiveRouteCapabilityAuthorizer for PermissiveLiveRouteCapabilityAuthorizer {
    fn check_capability<'a>(
        &'a self,
        _subject: &'a davenda_auth::DefaultSubject,
        _capability: davenda_auth::Capability,
        _object: &'a davenda_auth::Entity,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<bool, RuntimeServerError>> + Send + 'a>,
    > {
        Box::pin(async { Ok(true) })
    }
}
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod extensions;
mod surfaces;

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
enabled = ["cms-pages", "admin-shell"]

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

#[derive(Debug, Clone)]
struct SelectedAuthModelPackage {
    manifest: AuthModelManifest,
    schema: zanzibar::Schema,
    capability_bindings: std::collections::HashMap<Capability, CapabilityBinding>,
}

impl SelectedAuthModelPackage {
    fn new(name: &str, mode: PackageMode) -> Self {
        let mut manifest = davenda_auth::default_manifest();
        manifest.name = name.to_string();
        manifest.mode = mode;
        Self {
            manifest,
            schema: default_schema(),
            capability_bindings: default_capability_bindings(),
        }
    }
}

impl AuthModelPackage for SelectedAuthModelPackage {
    fn manifest(&self) -> &AuthModelManifest {
        &self.manifest
    }

    fn schema(&self) -> &zanzibar::Schema {
        &self.schema
    }

    fn capability_bindings(&self) -> &std::collections::HashMap<Capability, CapabilityBinding> {
        &self.capability_bindings
    }
}

fn single_node_valid_config() -> PlatformConfig {
    PlatformConfig::from_toml_str(VALID_CONFIG).unwrap()
}

fn config_with_auth_package(package: &str) -> PlatformConfig {
    PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "package = \"platform-default-auth\"",
        &format!("package = \"{package}\""),
    ))
    .unwrap()
}

fn config_with_outbound_http() -> PlatformConfig {
    PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "\n[jobs]\nbackend = \"redis\"\n",
        "\n[[wasm.outbound_http]]\nintegration = \"crm\"\nendpoint = \"https://crm.example.com/api\"\n\n[jobs]\nbackend = \"redis\"\n",
    ))
    .unwrap()
}

fn installed_admin_widget_extension() -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("admin.waitlist").unwrap(),
            "Waitlist Dashboard Widgets",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::AdminWidget),
            vec![
                HandlerManifest::new(
                    HandlerId::new("waitlist-summary").unwrap(),
                    "exports.waitlist_summary",
                    ExtensionPoint::AdminWidget(
                        AdminWidgetExtensionPoint::new("admin.dashboard.summary").unwrap(),
                    ),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::AuthCheck,
                        HostCapabilityGrant::DataRead {
                            resource: "events.waitlist".to_string(),
                        },
                    ]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "showcase-events",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-summary").unwrap(),
                HostGrantSet::from_grants([
                    HostCapabilityGrant::AuthCheck,
                    HostCapabilityGrant::DataRead {
                        resource: "events.waitlist".to_string(),
                    },
                ]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn installed_render_hook_extension() -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("cms.loyalty").unwrap(),
            "CMS Loyalty Fragments",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::RenderHook),
            vec![
                HandlerManifest::new(
                    HandlerId::new("loyalty-badge").unwrap(),
                    "exports.loyalty_badge",
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
        ExtensionInstallation::new(
            "showcase-events",
            vec![HandlerInstallation::new(
                HandlerId::new("loyalty-badge").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                    slot: "cms.page.render".to_string(),
                }]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn installed_job_extension() -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("ops.search.worker").unwrap(),
            "Ops Search Worker",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Job),
            vec![
                HandlerManifest::new(
                    HandlerId::new("search-adapter").unwrap(),
                    "exports.search_adapter",
                    ExtensionPoint::Job(
                        JobExtensionPoint::new("ops.search.adapter", "jobs.work").unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::EnqueueJob {
                        queue: "jobs.work".to_string(),
                    }]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "showcase-events",
            vec![HandlerInstallation::new(
                HandlerId::new("search-adapter").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::EnqueueJob {
                    queue: "jobs.work".to_string(),
                }]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn installed_page_extension_for_app(route: &str, customer_app_id: &str) -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("account.runtime").unwrap(),
            "Account Runtime Page",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Page),
            vec![
                HandlerManifest::new(
                    HandlerId::new("account-dashboard").unwrap(),
                    "exports.account_dashboard",
                    ExtensionPoint::Page(
                        PageExtensionPoint::new(route, [davenda_wasm::HttpMethod::Get]).unwrap(),
                    ),
                    HostGrantSet::new(),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            customer_app_id,
            vec![HandlerInstallation::new(
                HandlerId::new("account-dashboard").unwrap(),
                HostGrantSet::new(),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn installed_page_extension_for_app_with_artifact(
    extension_dir: &Path,
    route: &str,
    customer_app_id: &str,
) -> InstalledExtension {
    write_guest_artifact_with_typed_output(
        extension_dir,
        "account.runtime",
        "exports.account_dashboard",
        &[],
        InvocationOutcome::Page,
        page_extension_typed_output(),
    );
    installed_page_extension_for_app(route, customer_app_id)
}

fn installed_webhook_extension() -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("commerce.payment.webhooks").unwrap(),
            "Commerce Payment Webhooks",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Webhook),
            vec![
                HandlerManifest::new(
                    HandlerId::new("payment-authorized").unwrap(),
                    "exports.payment_authorized",
                    ExtensionPoint::Webhook(
                        WebhookExtensionPoint::new(
                            "commerce.payment-provider",
                            "payment.authorized",
                        )
                        .unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::EnqueueJob {
                        queue: "jobs.work".to_string(),
                    }]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "showcase-events",
            vec![HandlerInstallation::new(
                HandlerId::new("payment-authorized").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::EnqueueJob {
                    queue: "jobs.work".to_string(),
                }]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn installed_admin_widget_extension_with_artifact(extension_dir: &Path) -> InstalledExtension {
    write_guest_artifact_with_typed_output(
        extension_dir,
        "admin.waitlist",
        "exports.waitlist_summary",
        &[],
        InvocationOutcome::AdminWidget,
        admin_widget_typed_output(),
    );
    installed_admin_widget_extension()
}

fn installed_render_hook_extension_with_artifact(extension_dir: &Path) -> InstalledExtension {
    write_guest_artifact_with_typed_output(
        extension_dir,
        "cms.loyalty",
        "exports.loyalty_badge",
        &[(0, 0)],
        InvocationOutcome::RenderHook,
        render_hook_typed_output(),
    );
    installed_render_hook_extension()
}

fn installed_api_extension_with_artifact(
    extension_dir: &Path,
    route: &str,
    customer_app_id: &str,
) -> InstalledExtension {
    write_guest_artifact_with_typed_output(
        extension_dir,
        "api.account",
        "exports.account_json",
        &[],
        InvocationOutcome::ApiJson,
        api_typed_output(),
    );

    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("api.account").unwrap(),
            "Account API",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Api),
            vec![
                HandlerManifest::new(
                    HandlerId::new("account-json").unwrap(),
                    "exports.account_json",
                    ExtensionPoint::Api(
                        ApiExtensionPoint::new(route, [davenda_wasm::HttpMethod::Get]).unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            customer_app_id,
            vec![HandlerInstallation::new(
                HandlerId::new("account-json").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn write_guest_artifact_with_typed_output(
    extension_dir: &Path,
    extension_id: &str,
    export: &str,
    host_calls: &[(i32, i64)],
    outcome: InvocationOutcome,
    typed_output: TypedExecutionOutput,
) {
    let wasm = wat::parse_str(guest_module_with_typed_output(
        export,
        host_calls,
        outcome,
        &Some(typed_output),
    ))
    .unwrap();
    let path = extension_dir.join(format!("{extension_id}.wasm"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, wasm).unwrap();
}

fn page_extension_typed_output() -> TypedExecutionOutput {
    let metadata = TypedMetadata::new()
        .with_title("Account Runtime Extension")
        .unwrap()
        .with_description("runtime extension output for the account page")
        .unwrap();
    let cache_hint = TypedCacheHint::new(
        CacheVisibility::Public,
        60,
        Some(30),
        true,
        false,
        false,
        ["account-runtime"],
    )
    .unwrap();
    TypedExecutionOutput::page(
        202,
        "<section id=\"runtime-extension\">Account runtime extension</section>",
        metadata,
        Some(cache_hint),
    )
    .unwrap()
}

fn render_hook_typed_output() -> TypedExecutionOutput {
    let metadata = TypedMetadata::new()
        .with_description("render hook output for loyalty badges")
        .unwrap();
    TypedExecutionOutput::render_hook(
        200,
        "<aside data-extension=\"cms.loyalty\">Loyalty badge</aside>",
        metadata,
        None,
    )
    .unwrap()
}

fn admin_widget_typed_output() -> TypedExecutionOutput {
    TypedExecutionOutput::admin_widget(
        200,
        "<section data-widget=\"waitlist-summary\">Waitlist widget</section>",
        TypedMetadata::new(),
        None,
    )
    .unwrap()
}

fn api_typed_output() -> TypedExecutionOutput {
    TypedExecutionOutput::api(
        200,
        BTreeMap::from([("extension".to_string(), "ok".to_string())]),
        TypedMetadata::new(),
        None,
    )
    .unwrap()
}

fn installed_scheduled_job_extension() -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("ops.search.nightly").unwrap(),
            "Ops Search Nightly",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::ScheduledJob),
            vec![
                HandlerManifest::new(
                    HandlerId::new("nightly-rebuild").unwrap(),
                    "exports.nightly_rebuild",
                    ExtensionPoint::ScheduledJob(
                        ScheduledJobExtensionPoint::new("ops.search.nightly", "0 3 * * *").unwrap(),
                    ),
                    HostGrantSet::from_grants([HostCapabilityGrant::EnqueueJob {
                        queue: "jobs.work".to_string(),
                    }]),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "showcase-events",
            vec![HandlerInstallation::new(
                HandlerId::new("nightly-rebuild").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::EnqueueJob {
                    queue: "jobs.work".to_string(),
                }]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

fn plan_browser_services() -> davenda_core::BrowserSecurityServices {
    RuntimeBuilder::new(
        PlatformConfig::from_toml_str(VALID_CONFIG).unwrap(),
        DefaultAuthModelPackage::default(),
    )
    .build()
    .unwrap()
    .browser
}

fn guest_module(export: &str, host_calls: &[(i32, i64)], outcome: InvocationOutcome) -> String {
    guest_module_with_typed_output(export, host_calls, outcome, &None)
}

fn guest_module_with_typed_output(
    export: &str,
    host_calls: &[(i32, i64)],
    outcome: InvocationOutcome,
    typed_output: &Option<TypedExecutionOutput>,
) -> String {
    let mut body = String::new();
    for (slot, metric) in host_calls {
        body.push_str(&format!(
            "    i32.const {slot}\n    i64.const {metric}\n    call $host_call\n    drop\n"
        ));
    }

    let typed_output_module = if let Some(output) = typed_output {
        let bytes = output.encode().unwrap();
        let packed = (bytes.len() as u64) << 32;
        format!(
            "            (memory (export \"memory\") 1)\n            (data (i32.const 0) \"{}\")\n            (func (export \"__davenda_typed_output\") (result i64)\n                i64.const {packed}\n            )\n",
            wat_string_literal(&bytes)
        )
    } else {
        String::new()
    };

    format!(
        "(module
            (import \"davenda\" \"host_call\" (func $host_call (param i32 i64) (result i32)))
{typed_output_module}            (func (export \"{export}\") (result i32)
{body}                i32.const {}
            )
        )",
        outcome.engine_code()
    )
}

fn wat_string_literal(bytes: &[u8]) -> String {
    let mut literal = String::with_capacity(bytes.len() * 4);
    for byte in bytes {
        literal.push_str(&format!("\\{:02x}", byte));
    }
    literal
}

fn page_template(namespace: TemplateNamespace, name: &str) -> TemplateDefinition {
    let title = ElementNode::new("title", vec![Node::value("route_name").unwrap()]).unwrap();
    let head = ElementNode::new("head", vec![Node::Element(title)]).unwrap();
    let main = ElementNode::new("main", vec![Node::value("path").unwrap()])
        .unwrap()
        .with_attribute(AttributeNode::dynamic_text("data-route", "route_name").unwrap())
        .with_attribute(AttributeNode::dynamic_text("data-template", "template_name").unwrap());
    let body = ElementNode::new("body", vec![Node::Element(main)]).unwrap();
    let html = ElementNode::new("html", vec![Node::Element(head), Node::Element(body)])
        .unwrap()
        .with_attribute(AttributeNode::dynamic_text("lang", "locale").unwrap());

    TemplateDefinition::layout(
        namespace,
        TemplateName::new(name).unwrap(),
        vec![Node::static_text("<!DOCTYPE html>"), Node::Element(html)],
    )
}

fn fragment_template(namespace: TemplateNamespace, name: &str) -> TemplateDefinition {
    let fragment = ElementNode::new("div", vec![Node::value("path").unwrap()])
        .unwrap()
        .with_attribute(AttributeNode::dynamic_text("id", "surface_id").unwrap())
        .with_attribute(AttributeNode::dynamic_text("data-route", "route_name").unwrap())
        .with_attribute(AttributeNode::dynamic_text("data-template", "template_name").unwrap());

    TemplateDefinition::fragment(
        namespace,
        TemplateName::new(name).unwrap(),
        vec![Node::Element(fragment)],
    )
}

fn unique_temp_extension_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("davenda-runtime-{label}-{unique}"))
}

fn config_with_extension_directory(dir: &Path) -> PlatformConfig {
    PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "directory = \"extensions\"",
        &format!("directory = \"{}\"", dir.display()),
    ))
    .unwrap()
}

fn config_with_app_name(app_name: &str) -> PlatformConfig {
    PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "name = \"showcase-events\"",
        &format!("name = \"{app_name}\""),
    ))
    .unwrap()
}

fn config_with_app_name_and_extension_directory(dir: &Path, app_name: &str) -> PlatformConfig {
    let config = VALID_CONFIG
        .replace(
            "name = \"showcase-events\"",
            &format!("name = \"{app_name}\""),
        )
        .replace(
            "directory = \"extensions\"",
            &format!("directory = \"{}\"", dir.display()),
        );
    PlatformConfig::from_toml_str(&config).unwrap()
}

#[derive(Debug)]
struct StaticManifestModule {
    manifest: ModuleManifest,
}

impl StaticManifestModule {
    fn new(manifest: ModuleManifest) -> Self {
        Self { manifest }
    }
}

impl PlatformModule for StaticManifestModule {
    fn manifest(&self) -> ModuleManifest {
        self.manifest.clone()
    }

    fn register(
        &self,
        _registry: &mut davenda_core::ServiceRegistry,
    ) -> Result<(), RegistrationError> {
        Ok(())
    }
}

fn external_tls_config() -> String {
    VALID_CONFIG.replace(
        "mode = \"acme\"\nchallenge = \"dns-01\"\nprovider = \"cloudflare-dns\"",
        "mode = \"external\"",
    )
}

fn cloudflare_origin_tls_config() -> String {
    VALID_CONFIG.replace(
        "mode = \"acme\"\nchallenge = \"dns-01\"\nprovider = \"cloudflare-dns\"",
        "mode = \"cloudflare-origin\"\nprovider = \"cloudflare-origin-ca\"",
    )
}

fn active_certificate(id: &str, hostname: &str) -> CertificateRecord {
    CertificateRecord::new(
        CertificateId::new(id).unwrap(),
        CertificateProviderKind::Acme,
        CertificateStatus::Active,
        CertificateFingerprint::new(format!("sha256:{id}")).unwrap(),
        TlsInstant::from_unix_seconds(1_000),
        TlsInstant::from_unix_seconds(4_000_000),
        SecretMaterialRef::new(format!("secrets/tls/{id}")).unwrap(),
        CertificateStateStore::SharedSecrets,
    )
    .with_binding(HostnameBinding::new(
        Hostname::new(hostname).unwrap(),
        CustomerAppId::new("showcase-events").unwrap(),
    ))
}

fn content_fingerprint(fill: char) -> ContentFingerprint {
    ContentFingerprint::new(FingerprintAlgorithm::Sha256, fill.to_string().repeat(64)).unwrap()
}

fn config_with_backend_secrets() -> String {
    let with_storage_secret = VALID_CONFIG.replace(
        "local_root = \"/tmp/davenda-runtime-tests\"",
        "local_root = \"/tmp/davenda-runtime-tests\"\nobject_store_secret = { kind = \"env\", var = \"OBJECT_STORE_URL\" }",
    );
    format!(
        "{with_storage_secret}\n[database]\nurl = {{ kind = \"env\", var = \"DATABASE_URL\" }}\n"
    )
}

fn config_with_wasm_secret_bindings() -> String {
    VALID_CONFIG.replace(
        "allow_network = false",
        "allow_network = false\nsecret_bindings = { api_token = { kind = \"env\", var = \"WASM_API_TOKEN\" } }",
    )
}

fn config_with_wasm_outbound_http() -> String {
    VALID_CONFIG.replace(
        "allow_network = false",
        "allow_network = false\n[[wasm.outbound_http]]\nintegration = \"crm\"\nendpoint = \"https://crm.example.com/api\"",
    )
}

fn cookie_value(set_cookie_header: &str) -> String {
    set_cookie_header
        .split(';')
        .next()
        .and_then(|cookie| cookie.split_once('='))
        .map(|(_, value)| value.to_string())
        .expect("set-cookie header should include a value")
}

fn tag(value: &str) -> InvalidationTag {
    InvalidationTag::new(value).unwrap()
}

#[test]
fn runtime_builder_creates_a_runtime_plan() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("events.show", HttpMethod::Get, "/events")
                .unwrap()
                .localized()
                .from_module("events"),
        )
        .with_route(
            RouteDefinition::new("admin.settings", HttpMethod::Get, "/admin/settings")
                .unwrap()
                .with_area(RouteArea::Admin)
                .requiring_session(),
        )
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(MembershipsModule::new())
        .with_module(EventsModule::new())
        .with_module(MediaModule::new())
        .build()
        .unwrap();

    assert_eq!(plan.auth_package_name, "platform-default-auth");
    assert_eq!(plan.tenant_id(), 101);
    assert_eq!(
        plan.cache_topology.l2(),
        Some(DistributedCacheBackend::Redis)
    );
    assert_eq!(plan.browser.sessions.session_cookie.name, "davenda_session");
    assert_eq!(plan.browser.csrf.field_name, "_csrf");
    assert!(
        plan.cli
            .registry
            .commands()
            .any(|command| command.path == vec!["tls".to_string(), "renew".to_string()])
    );
    assert_eq!(plan.data.driver, davenda_config::DatabaseDriver::Postgres);
    assert_eq!(plan.data.schema, "public");
    assert_eq!(plan.jobs.backend, davenda_config::JobBackend::Redis);
    assert_eq!(
        plan.jobs.topology.scheduled_queue.as_str(),
        "jobs.scheduled"
    );
    assert_eq!(plan.tls.mode, davenda_config::TlsMode::Acme);
    assert_eq!(
        plan.tls.provider.map(|provider| provider.to_string()),
        Some("cloudflare_dns".to_string())
    );
    assert!(plan.observability.telemetry.metrics_enabled);
    assert!(plan.observability.telemetry.trace.enabled);
    assert_eq!(
        plan.observability.readiness.overall_status(),
        davenda_observability::DependencyStatus::Healthy
    );
    assert_eq!(
        plan.http.middleware,
        vec![
            MiddlewareStage::TransportNormalization,
            MiddlewareStage::CustomerAppResolution,
            MiddlewareStage::TraceContext,
            MiddlewareStage::LocaleResolution,
            MiddlewareStage::SessionResolution,
            MiddlewareStage::BrowserPolicy,
            MiddlewareStage::ResponsePolicy,
        ]
    );
    assert_eq!(
        plan.http.resolve(
            &plan.config,
            HttpMethod::Get,
            "www.example.com",
            "/fr-FR/events"
        ),
        Some(ResolvedRoute {
            route_name: "events.show".to_string(),
            locale: Some("fr-FR".to_string()),
            auth: RouteAuthGate::Public,
            params: BTreeMap::new(),
        })
    );
    assert_eq!(
        plan.template
            .namespace_chain(Some(&TemplateNamespace::new("cms-pages").unwrap())),
        vec![
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateNamespace::new("cms-pages").unwrap(),
            TemplateNamespace::new("core").unwrap(),
        ]
    );
    assert_eq!(plan.wasm.extension_directory, "extensions");
    assert_eq!(
        plan.wasm
            .limits
            .for_point(ExtensionPointKind::Page)
            .max_runtime,
        Duration::from_millis(50)
    );
    assert!(
        plan.services
            .iter()
            .any(|service| service.id == "module.admin.shell")
    );
    assert!(
        plan.services
            .iter()
            .any(|service| service.id == "module.cms.pages")
    );
    assert!(
        plan.services
            .iter()
            .any(|service| service.id == "module.commerce.checkout")
    );
    assert!(
        plan.services
            .iter()
            .any(|service| service.id == "module.memberships.entitlements")
    );
    assert!(
        plan.services
            .iter()
            .any(|service| service.id == "module.events.bookings")
    );
    assert!(
        plan.services
            .iter()
            .any(|service| service.id == "module.media.assets")
    );
    assert_eq!(plan.modules.len(), 6);
    assert_eq!(plan.modules[0].name, "admin");
    assert_eq!(plan.modules[1].name, "cms");
    assert_eq!(plan.modules[2].name, "commerce");
    assert_eq!(plan.modules[3].name, "memberships");
    assert_eq!(plan.modules[4].name, "events");
    assert_eq!(plan.modules[5].name, "media");
    assert!(
        plan.install_migrations
            .ordered_steps()
            .iter()
            .any(|step| step.owner == davenda_data::MigrationOwner::Module("cms".to_string()))
    );
    assert!(plan.install_migrations.ordered_steps().iter().any(|step| {
        step.owner == davenda_data::MigrationOwner::Module("memberships".to_string())
    }));
    assert!(
        plan.module_jobs
            .iter()
            .any(|registered| registered.job.name == "events.reminders")
    );
    assert!(plan.module_event_subscriptions.iter().any(|registered| {
        registered.subscription.event == "commerce.order.paid" && registered.module == "memberships"
    }));
    assert!(plan.module_data_repositories.iter().any(|registered| {
        registered.module == "cms" && registered.contribution.id == "cms.pages.live"
    }));
    assert!(plan.module_data_repositories.iter().any(|registered| {
        registered.module == "commerce" && registered.contribution.id == "commerce.catalog.products"
    }));
    assert!(plan.module_data_repositories.iter().any(|registered| {
        registered.module == "events" && registered.contribution.id == "events.waitlist"
    }));
    assert!(plan.registered_runtime_jobs.iter().any(|registered| {
        registered.contract.name == "events.reminders"
            && registered.queue == plan.jobs.topology.scheduled_queue
    }));
    assert!(
        plan.registered_runtime_event_subscriptions
            .iter()
            .any(|registered| {
                registered.module == "memberships"
                    && registered.event_type.as_str() == "commerce.order.paid"
                    && registered.job_name == "memberships.entitlements.sync"
            })
    );
    assert!(
        plan.jobs_domain
            .handlers
            .iter()
            .any(|handler| handler.id.as_str() == "memberships.entitlements.sync")
    );
    assert!(plan.module_search_contributions.iter().any(|registered| {
        registered.module == "commerce" && registered.contribution.id == "search.catalog.products"
    }));
    assert!(plan.module_report_definitions.iter().any(|registered| {
        registered.module == "memberships"
            && registered.definition.id == "report.memberships.summary"
    }));
    assert!(plan.module_bulk_operations.iter().any(|registered| {
        registered.module == "events" && registered.definition.id == "bulk.events.check-in"
    }));
    assert!(
        plan.ops_catalog
            .reports
            .definition(&ReportId::new("report.memberships.summary").unwrap())
            .is_some()
    );
    assert!(
        plan.ops_catalog
            .bulk
            .definition(&BulkOperationId::new("bulk.events.check-in").unwrap())
            .is_some()
    );
}

#[test]
fn rejects_duplicate_route_names_for_the_same_method() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(RouteDefinition::new("events.show", HttpMethod::Get, "/events").unwrap())
        .with_route(
            RouteDefinition::new("events.show", HttpMethod::Get, "/events-duplicate").unwrap(),
        )
        .build()
        .unwrap_err();

    match error {
        RuntimeBuildError::Route(RouteBuildError::DuplicateRoute { name, method }) => {
            assert_eq!(name, "events.show");
            assert_eq!(method, HttpMethod::Get);
        }
        other => panic!("expected duplicate route error, got {other:?}"),
    }
}

#[test]
fn runtime_builder_rejects_duplicate_runtime_data_repositories() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let repository = davenda_core::DataRepositoryContribution::new(
        davenda_data::RepositorySpec::new(
            "shared.runtime.repo",
            davenda_data::TableName::new("davenda.shared_runtime_repo").unwrap(),
            vec![davenda_data::QueryField::new("id").unwrap()],
        )
        .unwrap(),
        davenda_core::DataRepositoryQueryProfile::new(
            davenda_data::PageRequest::new(0, 10).unwrap(),
            davenda_data::PublicationVisibility::PublishedOnly,
            davenda_data::QueryCacheScope::Public,
        ),
    );

    let first = StaticManifestModule::new(
        ModuleManifest::new("first").with_data_repositories(vec![repository.clone()]),
    );
    let second = StaticManifestModule::new(
        ModuleManifest::new("second").with_data_repositories(vec![repository]),
    );

    let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(first)
        .with_module(second)
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeBuildError::DuplicateDataRepository {
            repository,
            first_module,
            second_module,
        } if repository == "shared.runtime.repo"
            && first_module == "first"
            && second_module == "second"
    ));
}

#[test]
fn execute_request_derives_context_and_session_from_cookie() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-123")
        .unwrap();

    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/account")
                .unwrap()
                .with_session_cookie(session_cookie)
                .with_principal("user-1"),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    assert_eq!(execution.customer_app, "showcase-events");
    assert_eq!(execution.route.route_name, "account.dashboard");
    assert_eq!(execution.route_area, RouteArea::Account);
    assert_eq!(execution.locale, "en-GB");
    assert_eq!(execution.session.session_id.as_deref(), Some("session-123"));
    assert!(execution.session.resolved_from_cookie);
    assert_eq!(execution.cache, CacheDisposition::Private);
    assert_eq!(
        execution
            .cache_plan
            .headers
            .get("Cache-Control")
            .map(String::as_str),
        Some("private, max-age=60, stale-while-revalidate=30")
    );
    assert!(
        execution
            .cache_plan
            .headers
            .get("X-Davenda-Variation-Key")
            .is_some()
    );
    assert_eq!(execution.trace.transport_scheme, "https");
    assert_eq!(execution.middleware, plan.http.middleware);
    assert_eq!(
        execution.response,
        HandlerResponse::Page(PageResponse {
            template: "account/dashboard".to_string(),
            status: 200,
        })
    );
    assert!(execution.flash_messages.is_empty());
    assert!(execution.response_cookies.is_empty());
}

#[test]
fn browser_host_issues_rotates_and_revokes_server_side_sessions() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let mut host = plan.browser_host().unwrap();
    let cookie_secret = b"01234567012345670123456701234567";

    let issued = host
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-1")
                .unwrap(),
            cookie_secret,
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();
    assert!(issued.set_cookie_header.starts_with("davenda_session="));
    assert_eq!(
        host.session(&issued.record.session_id)
            .unwrap()
            .and_then(|record| record.principal_id),
        Some("member-1".to_string())
    );

    let rotated = host
        .rotate_session(
            &issued.record.session_id,
            cookie_secret,
            BrowserInstant::from_unix_seconds(120),
        )
        .unwrap();
    assert_ne!(rotated.issued.record.session_id, issued.record.session_id);
    assert_eq!(
        host.session(&issued.record.session_id)
            .unwrap()
            .map(|record| record.status_at(BrowserInstant::from_unix_seconds(121))),
        Some(BrowserSessionStatus::Revoked)
    );

    host.revoke_session(
        &rotated.issued.record.session_id,
        BrowserInstant::from_unix_seconds(130),
    )
    .unwrap();
    assert_eq!(
        host.session(&rotated.issued.record.session_id)
            .unwrap()
            .map(|record| record.status_at(BrowserInstant::from_unix_seconds(131))),
        Some(BrowserSessionStatus::Revoked)
    );
}

#[test]
fn browser_host_keeps_memory_sessions_local_to_each_clone() {
    let config = PlatformConfig::from_toml_str(
        &VALID_CONFIG.replace("store = \"redis\"", "store = \"memory\""),
    )
    .unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let mut left =
        BrowserHost::local_for_testing(plan.config.app.name.clone(), plan.browser.clone());
    let mut right = left.clone();
    assert_eq!(left.session_store_kind(), SessionStoreBackendKind::Local);
    assert!(!left.session_store_is_shared());

    let issued = left
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-2")
                .unwrap(),
            b"01234567012345670123456701234567",
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();

    assert!(right.session(&issued.record.session_id).unwrap().is_none());
    assert!(
        right
            .resolve_request(
                &RequestInput::new(HttpMethod::Get, "www.example.com", "/account").unwrap(),
                b"01234567012345670123456701234567",
                BrowserInstant::from_unix_seconds(150),
            )
            .unwrap()
            .session
            .session_id
            .is_none()
    );
}

#[test]
fn server_host_rejects_memory_session_stores_for_live_wiring() {
    let config = PlatformConfig::from_toml_str(
        &VALID_CONFIG.replace("store = \"redis\"", "store = \"memory\""),
    )
    .unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let error = plan
        .server_host(
            &crate::server::StaticSecretResolver::new(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeServerError::BrowserHostBuild(
            BrowserHostBuildError::MemoryStoreRequiresTestOnlyBrowserHost
        )
    ));
}

#[test]
fn browser_host_shares_distributed_sessions_by_default_within_a_plan() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let mut left = plan.browser_host().unwrap();
    let right = plan.browser_host().unwrap();
    assert_eq!(left.session_store_kind(), SessionStoreBackendKind::Redis);
    assert!(left.session_store_is_shared());

    let issued = left
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-3")
                .unwrap(),
            b"01234567012345670123456701234567",
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();

    assert_eq!(
        right
            .session(&issued.record.session_id)
            .unwrap()
            .and_then(|record| record.principal_id),
        Some("member-3".to_string())
    );
}

#[test]
fn cache_host_shares_distributed_state_by_default_within_a_plan() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .build()
        .unwrap();
    let mut left = plan.cache_host().unwrap();
    let mut right = plan.cache_host().unwrap();

    assert_eq!(left.metrics(), CacheMetrics::default());

    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/en-GB/pages/home").unwrap(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let stored_key = left
        .store_execution(
            &execution,
            "<html>shared</html>",
            CacheInstant::from_unix_seconds(100),
        )
        .expect("page execution is cacheable");

    let lookup = right
        .lookup_execution(&execution, CacheInstant::from_unix_seconds(110))
        .expect("page execution has an application cache plan");
    assert_eq!(lookup.state, CacheLookupState::Fresh);
    assert_eq!(
        lookup.entry.as_ref().map(|entry| entry.key.clone()),
        Some(stored_key)
    );
}

#[test]
fn browser_host_shares_distributed_sessions_when_reusing_an_explicit_client() {
    let services = plan_browser_services();
    let client = DistributedSessionStoreClient::local_for_testing(SessionStoreBackendKind::Redis);
    let mut left = BrowserHost::with_session_store_client(
        "showcase-events".to_string(),
        services.clone(),
        client.clone(),
    )
    .unwrap();
    let right =
        BrowserHost::with_session_store_client("showcase-events".to_string(), services, client)
            .unwrap();
    let cookie_secret = b"01234567012345670123456701234567";

    let issued = left
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-3")
                .unwrap(),
            cookie_secret,
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();

    assert_eq!(left.session_store_kind(), SessionStoreBackendKind::Redis);
    assert!(left.session_store_is_shared());
    assert_eq!(
        right
            .session(&issued.record.session_id)
            .unwrap()
            .and_then(|record| record.principal_id),
        Some("member-3".to_string())
    );
}

#[test]
fn cache_runtime_shares_distributed_state_when_reusing_an_explicit_backend() {
    let topology = CacheTopology::with_redis();
    let planner = CachePlanner::new(topology);
    let shared_runtime = crate::plan::shared_cache_runtime_for_test(
        CacheBackendKind::Redis,
        "runtime-cache-shared-test".to_string(),
    );
    let adapter = CacheBackendAdapter::with_shared_runtime(topology, shared_runtime);
    let mut left = CacheRuntime::with_backend(topology, adapter.clone());
    let mut right = CacheRuntime::with_backend(topology, adapter);

    assert_eq!(left.backend_kind(), CacheBackendKind::Redis);

    let app_policy = ApplicationCachePolicy::new(
        CacheScope::public()
            .with_site("main")
            .unwrap()
            .with_locale("en-GB")
            .unwrap(),
        FreshnessPolicy::new(Duration::from_secs(300), Some(Duration::from_secs(30))).unwrap(),
        InvalidationSet::from_tags([tag("page:42"), tag("nav:main")]),
    )
    .unwrap();
    let http_policy = HttpCachePolicy::new(
        CacheScope::public()
            .with_site("main")
            .unwrap()
            .with_locale("en-GB")
            .unwrap(),
        Some(FreshnessPolicy::new(Duration::from_secs(60), Some(Duration::from_secs(15))).unwrap()),
        ResponseValidators::default(),
        InvalidationSet::from_tags([tag("page:42")]),
    )
    .unwrap();
    let plan = planner
        .plan(
            CachePlanRequest::new(
                CacheNamespace::new("cms.page").unwrap(),
                "page:42",
                http_policy,
            )
            .unwrap()
            .with_application_policy(app_policy),
        )
        .unwrap();

    left.insert(
        plan.application().unwrap(),
        "<html>shared</html>",
        CacheInstant::from_unix_seconds(100),
    );

    let lookup = right.lookup(
        plan.application().unwrap().key(),
        CacheInstant::from_unix_seconds(110),
    );
    assert_eq!(lookup.state, CacheLookupState::Fresh);
}

#[test]
fn execute_browser_request_uses_server_side_session_resolution_and_flash_transport() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .build()
        .unwrap();
    let mut host = plan.browser_host().unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";

    let issued = host
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-1")
                .unwrap(),
            cookie_secret,
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();
    let flash_cookie = host
        .issue_flash_cookie(
            cookie_secret,
            &[
                FlashMessage::new(FlashLevel::Success, "Saved changes").unwrap(),
                FlashMessage::new(FlashLevel::Info, "Ready for checkout").unwrap(),
            ],
        )
        .unwrap();

    let execution = plan
        .execute_browser_request(
            &mut host,
            RequestInput::new(HttpMethod::Get, "www.example.com", "/account")
                .unwrap()
                .with_session_cookie(issued.cookie_value.clone())
                .with_flash_cookie(cookie_value(&flash_cookie)),
            cookie_secret,
            csrf_secret,
            BrowserInstant::from_unix_seconds(150),
        )
        .unwrap();

    assert_eq!(
        execution.session.session_id.as_deref(),
        Some(issued.record.session_id.as_str())
    );
    assert!(execution.session.resolved_from_cookie);
    assert_eq!(
        execution.principal.principal_id.as_deref(),
        Some("member-1")
    );
    assert_eq!(
        execution.flash_messages,
        vec![
            FlashMessage::new(FlashLevel::Success, "Saved changes").unwrap(),
            FlashMessage::new(FlashLevel::Info, "Ready for checkout").unwrap(),
        ]
    );
    assert_eq!(execution.response_cookies.len(), 2);
    assert!(
        execution
            .response_cookies
            .iter()
            .any(|header| header.starts_with("davenda_session="))
    );
    assert!(
        execution
            .response_cookies
            .iter()
            .any(|header| header.starts_with("davenda_flash=") && header.contains("Max-Age=0"))
    );
}

#[test]
fn execute_browser_request_rejects_expired_server_side_sessions() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("account.dashboard", HttpMethod::Get, "/account")
                .unwrap()
                .with_area(RouteArea::Account)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::page("account.dashboard", "account/dashboard").unwrap())
        .build()
        .unwrap();
    let mut host = plan.browser_host().unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let issued = host
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-1")
                .unwrap(),
            cookie_secret,
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();

    let error = plan
        .execute_browser_request(
            &mut host,
            RequestInput::new(HttpMethod::Get, "www.example.com", "/account")
                .unwrap()
                .with_session_cookie(issued.cookie_value.clone()),
            cookie_secret,
            csrf_secret,
            BrowserInstant::from_unix_seconds(4_000),
        )
        .unwrap_err();

    assert_eq!(
        error,
        RequestExecutionError::ExpiredSession {
            session_id: issued.record.session_id.clone(),
        }
    );
    assert!(host.session(&issued.record.session_id).unwrap().is_none());
}

#[test]
fn execute_browser_request_supports_csrf_for_host_managed_sessions() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("cms.publish", HttpMethod::Post, "/admin/pages/publish")
                .unwrap()
                .with_area(RouteArea::Admin)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::redirect("cms.publish", "/admin/pages").unwrap())
        .build()
        .unwrap();
    let mut host = plan.browser_host().unwrap();
    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let issued = host
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("editor-1")
                .unwrap(),
            cookie_secret,
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();
    let token = host
        .issue_csrf_token(csrf_secret, &issued.record.session_id, "cms.publish")
        .unwrap();

    let execution = plan
        .execute_browser_request(
            &mut host,
            RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
                .unwrap()
                .with_session_cookie(issued.cookie_value)
                .with_csrf_token(token),
            cookie_secret,
            csrf_secret,
            BrowserInstant::from_unix_seconds(110),
        )
        .unwrap();

    assert_eq!(execution.cache, CacheDisposition::Uncacheable);
    assert_eq!(
        execution.response,
        HandlerResponse::Redirect(RedirectResponse {
            location: "/admin/pages".to_string(),
            status: 303,
        })
    );
    assert!(
        execution
            .response_cookies
            .iter()
            .any(|header| header.starts_with("davenda_session="))
    );
}

#[test]
fn execute_request_enforces_csrf_for_state_changing_browser_routes() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("cms.publish", HttpMethod::Post, "/admin/pages/publish")
                .unwrap()
                .with_area(RouteArea::Admin)
                .requiring_session(),
        )
        .with_handler(HandlerDefinition::redirect("cms.publish", "/admin/pages").unwrap())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-123")
        .unwrap();

    let missing_token = plan.execute_request(
        RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
            .unwrap()
            .with_session_cookie(session_cookie.clone()),
        cookie_secret,
        csrf_secret,
    );
    assert_eq!(
        missing_token.unwrap_err(),
        RequestExecutionError::MissingCsrfToken {
            route: "cms.publish".to_string(),
        }
    );

    let token = plan
        .browser
        .csrf
        .issue_token(csrf_secret, "session-123", "cms.publish")
        .unwrap();
    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/pages/publish")
                .unwrap()
                .with_session_cookie(session_cookie)
                .with_csrf_token(token),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    assert_eq!(execution.cache, CacheDisposition::Uncacheable);
    assert_eq!(
        execution
            .cache_plan
            .headers
            .get("Cache-Control")
            .map(String::as_str),
        Some("no-store")
    );
    assert!(execution.cache_plan.plan.application().is_none());
    assert_eq!(execution.route.route_name, "cms.publish");
    assert_eq!(
        execution.response,
        HandlerResponse::Redirect(RedirectResponse {
            location: "/admin/pages".to_string(),
            status: 303,
        })
    );
}

#[test]
fn execute_request_requires_capability_for_capability_gated_routes() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("cms.preview", HttpMethod::Get, "/admin/pages/preview")
                .unwrap()
                .with_area(RouteArea::Admin)
                .requiring_capability(davenda_auth::Capability::CmsPageRead),
        )
        .with_handler(
            HandlerDefinition::fragment("cms.preview", "cms/preview", "preview-pane").unwrap(),
        )
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-123")
        .unwrap();

    let denied = plan.execute_request(
        RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/pages/preview")
            .unwrap()
            .with_session_cookie(session_cookie.clone()),
        cookie_secret,
        csrf_secret,
    );
    assert_eq!(
        denied.unwrap_err(),
        RequestExecutionError::CapabilityRequired {
            route: "cms.preview".to_string(),
            capability: davenda_auth::Capability::CmsPageRead,
        }
    );

    let allowed = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/pages/preview")
                .unwrap()
                .with_session_cookie(session_cookie)
                .grant_capability(davenda_auth::Capability::CmsPageRead),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    assert_eq!(allowed.route.route_name, "cms.preview");
    assert_eq!(allowed.cache, CacheDisposition::Private);
    assert_eq!(
        allowed
            .cache_plan
            .headers
            .get("Cache-Control")
            .map(String::as_str),
        Some("private, max-age=30, stale-while-revalidate=15")
    );
    assert_eq!(
        allowed.response,
        HandlerResponse::Fragment(FragmentResponse {
            template: "cms/preview".to_string(),
            fragment_id: "preview-pane".to_string(),
        })
    );
}

#[test]
fn render_fragment_response_fails_closed_when_the_template_is_missing() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("missing.preview", HttpMethod::Get, "/missing/preview")
                .unwrap()
                .localized(),
        )
        .with_handler(
            HandlerDefinition::fragment("missing.preview", "missing/preview", "preview-pane")
                .unwrap(),
        )
        .build()
        .unwrap();

    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/en-GB/missing/preview")
                .unwrap(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    let fragment = match &execution.response {
        HandlerResponse::Fragment(fragment) => fragment,
        _ => panic!("expected fragment handler response"),
    };

    let error = plan
        .render_fragment_response(&execution, fragment)
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeRenderError::Template(TemplateModelError::TemplateNotFound { .. })
    ));
}

#[test]
fn cache_host_stores_and_revalidates_public_route_responses() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("events.public", HttpMethod::Get, "/events")
                .unwrap()
                .localized(),
        )
        .with_handler(HandlerDefinition::page("events.public", "events/list").unwrap())
        .build()
        .unwrap();
    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/en-GB/events").unwrap(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    assert_eq!(execution.cache, CacheDisposition::Public);
    assert_eq!(
        execution
            .cache_plan
            .headers
            .get("Cache-Control")
            .map(String::as_str),
        Some("public, max-age=300, stale-while-revalidate=30")
    );
    assert!(execution.cache_plan.headers.get("Surrogate-Key").is_some());

    let mut host = plan.cache_host().unwrap();
    assert!(
        host.lookup_execution(&execution, CacheInstant::from_unix_seconds(100))
            .is_some_and(|lookup| lookup.state == CacheLookupState::Miss)
    );

    let fill = host
        .begin_fill(&execution, "renderer-1")
        .expect("public route should be application-cacheable");
    assert!(matches!(fill, FillDecision::Start(_)));
    let key = host
        .store_execution(
            &execution,
            "<html>events</html>",
            CacheInstant::from_unix_seconds(100),
        )
        .expect("public route should store into the cache");
    host.complete_fill(&fill).unwrap();

    let fresh = host
        .lookup_execution(&execution, CacheInstant::from_unix_seconds(110))
        .expect("public route should look up through the cache host");
    assert_eq!(fresh.state, CacheLookupState::Fresh);
    assert_eq!(
        fresh.entry.as_ref().map(|entry| entry.key.clone()),
        Some(key.clone())
    );

    let invalidated = host.invalidate(execution.cache_plan.plan.application().unwrap().tags());
    assert_eq!(invalidated, vec![key]);
}

#[test]
fn runtime_plan_materializes_shared_backend_clients_from_config_secrets() {
    let config = PlatformConfig::from_toml_str(&config_with_backend_secrets()).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = StaticSecretResolver::new()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "DATABASE_URL".to_string(),
            },
            "postgres://platform:secret@db.internal/platform",
        )
        .unwrap()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "OBJECT_STORE_URL".to_string(),
            },
            "https://s3.internal/runtime",
        )
        .unwrap();

    let backends = plan.shared_backend_clients(&resolver).unwrap();
    assert_eq!(
        backends.database.driver,
        davenda_config::DatabaseDriver::Postgres
    );
    assert_eq!(
        backends.database.url.as_deref(),
        Some("postgres://platform:secret@db.internal/platform")
    );
    assert_eq!(
        backends
            .distributed_cache
            .as_ref()
            .map(|cache| cache.backend),
        Some(DistributedCacheBackend::Redis)
    );
    assert_eq!(backends.jobs.backend, davenda_config::JobBackend::Redis);
    assert_eq!(
        backends
            .object_store
            .as_ref()
            .and_then(|store| store.credential_reference.as_deref()),
        Some("https://s3.internal/runtime")
    );
}

#[test]
fn runtime_plan_resolves_wasm_secret_bindings_from_config_secrets() {
    let config = PlatformConfig::from_toml_str(&config_with_wasm_secret_bindings()).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let resolver = StaticSecretResolver::new()
        .with_secret(
            davenda_config::SecretRef::Env {
                var: "WASM_API_TOKEN".to_string(),
            },
            "runtime-secret",
        )
        .unwrap();

    let secrets = plan.wasm_secret_values(&resolver).unwrap();

    assert_eq!(
        secrets.get("api_token"),
        Some(&"runtime-secret".to_string())
    );
}

#[test]
fn runtime_plan_exposes_declared_wasm_outbound_http_endpoints() {
    let config = PlatformConfig::from_toml_str(&config_with_wasm_outbound_http()).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let approved = plan.approved_outbound_http_endpoints();

    assert_eq!(approved.len(), 1);
    assert_eq!(
        approved
            .get("crm")
            .map(|endpoint| endpoint.as_str())
            .as_deref(),
        Some("https://crm.example.com/api")
    );
}

#[test]
fn runtime_plan_rejects_missing_wasm_secret_bindings() {
    let config = PlatformConfig::from_toml_str(&config_with_wasm_secret_bindings()).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let error = plan
        .wasm_secret_values(&crate::server::StaticSecretResolver::new())
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeServerError::Secret(crate::server::SecretResolutionError::MissingSecret {
            reference,
        }) if reference == "env:WASM_API_TOKEN"
    ));
}

#[test]
fn live_http_request_adapter_extracts_runtime_headers_and_cookies() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let request = Request::builder()
        .method("POST")
        .uri("/admin/pages/publish")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .header("x-request-id", "req-live-1")
        .header("x-csrf-token", "csrf-123")
        .header("x-davenda-maintenance-bypass", "ops-bypass")
        .header(
            "cookie",
            "davenda_session=v1.session.sig; davenda_flash=v1.flash.sig",
        )
        .body(Body::empty())
        .unwrap();

    let live = LiveHttpRequest::from_request(
        &request,
        &plan.browser,
        &plan.config.server,
        Some(SocketAddr::from(([10, 0, 0, 42], 443))),
    )
    .unwrap();
    assert_eq!(live.method, HttpMethod::Post);
    assert_eq!(live.host, "www.example.com");
    assert_eq!(live.forwarded_proto.as_deref(), Some("https"));
    assert_eq!(live.request_id.as_deref(), Some("req-live-1"));
    assert_eq!(live.session_cookie.as_deref(), Some("v1.session.sig"));
    assert_eq!(live.flash_cookie.as_deref(), Some("v1.flash.sig"));
    assert_eq!(live.csrf_token.as_deref(), Some("csrf-123"));
    assert_eq!(live.maintenance_bypass_token.as_deref(), Some("ops-bypass"));
}

#[tokio::test]
async fn server_host_ignores_forwarded_metadata_from_untrusted_peers() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("events.list", HttpMethod::Get, "/events")
                .unwrap()
                .localized(),
        )
        .with_handler(HandlerDefinition::page("events.list", "events/list").unwrap())
        .build()
        .unwrap();

    let request = Request::builder()
        .method("GET")
        .uri("/events")
        .header("host", "www.example.com")
        .header("x-forwarded-proto", "https")
        .body(Body::empty())
        .unwrap();

    let live = LiveHttpRequest::from_request(
        &request,
        &plan.browser,
        &plan.config.server,
        Some(SocketAddr::from(([192, 168, 1, 42], 443))),
    )
    .unwrap();

    assert_eq!(live.forwarded_proto, None);
    assert_eq!(live.scheme, "http");
}

mod server;

#[test]
fn execute_request_blocks_routes_when_feature_flag_is_disabled() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let feature_flag = FeatureFlag::new("beta-events", false)
        .unwrap()
        .with_rule(
            davenda_observability::FlagTarget::CustomerApp(
                FlagCustomerAppId::new("other-app").unwrap(),
            ),
            true,
        )
        .unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new("events.beta", HttpMethod::Get, "/events/beta")
                .unwrap()
                .with_feature_flag("beta-events"),
        )
        .with_handler(HandlerDefinition::page("events.beta", "events/beta").unwrap())
        .with_feature_flag(feature_flag)
        .build()
        .unwrap();

    let error = plan.execute_request(
        RequestInput::new(HttpMethod::Get, "www.example.com", "/events/beta").unwrap(),
        b"01234567012345670123456701234567",
        b"76543210765432107654321076543210",
    );

    assert_eq!(
        error.unwrap_err(),
        RequestExecutionError::FeatureFlagDisabled {
            route: "events.beta".to_string(),
            feature_flag: "beta-events".to_string(),
        }
    );
}

#[test]
fn execute_request_respects_maintenance_mode_with_operator_bypass() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let maintenance = davenda_observability::MaintenanceMode {
        enabled: true,
        audience: MaintenanceAudience::CustomerApp(
            FlagCustomerAppId::new("showcase-events").unwrap(),
        ),
        impact: MaintenanceImpact::MutatingTrafficOnly,
        bypass_token: Some("ops-bypass".to_string()),
        allowed_background_work: BTreeSet::new(),
    };
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_route(
            RouteDefinition::new(
                "admin.bulk-publish",
                HttpMethod::Post,
                "/admin/bulk/publish",
            )
            .unwrap()
            .with_area(RouteArea::Admin)
            .requiring_session(),
        )
        .with_handler(
            HandlerDefinition::json(
                "admin.bulk-publish",
                BTreeMap::from([("status".to_string(), "queued".to_string())]),
            )
            .unwrap(),
        )
        .with_maintenance_mode(maintenance)
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-123")
        .unwrap();
    let token = plan
        .browser
        .csrf
        .issue_token(csrf_secret, "session-123", "admin.bulk-publish")
        .unwrap();

    let blocked = plan.execute_request(
        RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/bulk/publish")
            .unwrap()
            .with_session_cookie(session_cookie.clone())
            .with_csrf_token(token.clone()),
        cookie_secret,
        csrf_secret,
    );
    assert_eq!(
        blocked.unwrap_err(),
        RequestExecutionError::MaintenanceMode {
            route: "admin.bulk-publish".to_string(),
        }
    );

    let allowed = plan
        .execute_request(
            RequestInput::new(HttpMethod::Post, "www.example.com", "/admin/bulk/publish")
                .unwrap()
                .with_session_cookie(session_cookie)
                .with_csrf_token(token)
                .with_maintenance_bypass_token("ops-bypass"),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();
    assert_eq!(
        allowed.response,
        HandlerResponse::Json(JsonResponse {
            status: 200,
            payload: BTreeMap::from([("status".to_string(), "queued".to_string())]),
        })
    );
}

#[test]
fn runtime_builder_rejects_missing_required_module_dependencies() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(MembershipsModule::new())
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeBuildError::ModuleInstallation(ModuleInstallationError::MissingModuleDependency {
            module,
            dependency,
        }) if module == "memberships" && dependency == "commerce"
    ));
}

#[test]
fn runtime_builder_materializes_jobs_domain_for_module_subscriptions() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(MembershipsModule::new())
        .with_module(EventsModule::new())
        .build()
        .unwrap();

    assert!(plan.registered_runtime_jobs.iter().any(|job| {
        job.contract.name == "memberships.entitlements.sync"
            && job.queue == plan.jobs.topology.domain_events_queue
    }));
    assert!(
        plan.registered_runtime_event_subscriptions
            .iter()
            .any(|subscription| {
                subscription.job_name == "memberships.entitlements.sync"
                    && subscription.event_type.as_str() == "commerce.order.paid"
            })
    );
    assert!(plan.jobs_domain.validate().is_ok());
}

#[test]
fn runtime_backend_materializer_uses_shared_jobs_backends_even_when_not_flagged_shared() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .build()
        .unwrap();

    let materializer = crate::backends::RuntimeBackendMaterializer::new(
        plan.shared_backend_namespace(),
        SharedBackendClients {
            database: DatabaseClientTarget {
                driver: davenda_config::DatabaseDriver::Postgres,
                url: None,
                min_connections: 1,
                max_connections: 1,
                statement_timeout_secs: 30,
            },
            distributed_cache: None,
            jobs: JobsClientTarget {
                backend: davenda_config::JobBackend::Redis,
                shared: false,
            },
            session_store: None,
            object_store: None,
        },
    );

    let mut left = materializer.jobs_coordinator(&plan.config.app.name, &plan.jobs);
    let mut right = materializer.jobs_coordinator(&plan.config.app.name, &plan.jobs);
    let spec = JobSpec::new(
        JobId::new("job-shared").unwrap(),
        JobName::new("shared.jobs.test").unwrap(),
        plan.jobs.topology.work_queue.clone(),
        "shared jobs test",
    )
    .unwrap();

    left.enqueue(spec, JobInstant::from_unix_seconds(100))
        .unwrap();
    right.refresh();

    assert_eq!(right.ready_jobs().len(), 1);
    assert_eq!(right.ready_jobs()[0].spec.job_id.as_str(), "job-shared");
}

#[test]
fn runtime_backend_materializer_reuses_explicit_session_runtime_for_browser_hosts() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let materializer = crate::backends::RuntimeBackendMaterializer::new(
        plan.shared_backend_namespace(),
        SharedBackendClients {
            database: DatabaseClientTarget {
                driver: davenda_config::DatabaseDriver::Postgres,
                url: None,
                min_connections: 1,
                max_connections: 1,
                statement_timeout_secs: 30,
            },
            distributed_cache: None,
            jobs: JobsClientTarget {
                backend: davenda_config::JobBackend::Redis,
                shared: true,
            },
            session_store: Some(SessionStoreClientTarget {
                kind: SessionStoreBackendKind::Redis,
                shared: true,
            }),
            object_store: None,
        },
    );

    let mut left = materializer
        .browser_host(plan.config.app.name.clone(), plan.browser.clone())
        .unwrap();
    let right = materializer
        .browser_host(plan.config.app.name.clone(), plan.browser.clone())
        .unwrap();

    let issued = left
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-runtime")
                .unwrap(),
            b"01234567012345670123456701234567",
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();

    assert_eq!(
        right
            .session(&issued.record.session_id)
            .unwrap()
            .and_then(|record| record.principal_id),
        Some("member-runtime".to_string())
    );
}

#[test]
fn runtime_backend_materializer_shares_session_state_across_instances() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();

    let clients = SharedBackendClients {
        database: DatabaseClientTarget {
            driver: davenda_config::DatabaseDriver::Postgres,
            url: None,
            min_connections: 1,
            max_connections: 1,
            statement_timeout_secs: 30,
        },
        distributed_cache: None,
        jobs: JobsClientTarget {
            backend: davenda_config::JobBackend::Redis,
            shared: true,
        },
        session_store: Some(SessionStoreClientTarget {
            kind: SessionStoreBackendKind::Redis,
            shared: true,
        }),
        object_store: None,
    };
    let left_materializer = crate::backends::RuntimeBackendMaterializer::new(
        plan.shared_backend_namespace(),
        clients.clone(),
    );
    let right_materializer =
        crate::backends::RuntimeBackendMaterializer::new(plan.shared_backend_namespace(), clients);

    let mut left = left_materializer
        .browser_host(plan.config.app.name.clone(), plan.browser.clone())
        .unwrap();
    let right = right_materializer
        .browser_host(plan.config.app.name.clone(), plan.browser.clone())
        .unwrap();

    let issued = left
        .issue_session(
            SessionIssueRequest::new()
                .for_principal("member-isolated")
                .unwrap(),
            b"01234567012345670123456701234567",
            BrowserInstant::from_unix_seconds(100),
        )
        .unwrap();

    assert_eq!(
        right
            .session(&issued.record.session_id)
            .unwrap()
            .and_then(|record| record.principal_id),
        Some("member-isolated".to_string())
    );
}

#[test]
fn jobs_host_dispatches_domain_events_and_retries_declared_jobs() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(MembershipsModule::new())
        .build()
        .unwrap();
    let mut host = plan.jobs_host("scheduler-a").unwrap();

    let dispatch = host
        .emit_domain_event(
            DomainEventDispatchRequest::new(
                "commerce.order.paid",
                "order",
                "order-42",
                "membership checkout completed",
            )
            .unwrap(),
            JobInstant::from_unix_seconds(200),
        )
        .unwrap();

    assert_eq!(dispatch.event_type.as_str(), "commerce.order.paid");
    assert_eq!(dispatch.enqueued_jobs.len(), 2);
    assert_eq!(host.coordinator().ready_jobs().len(), 2);
    assert!(host.coordinator().ready_jobs().iter().any(|record| {
        record.spec.job_name.as_str() == "event-handler:memberships.entitlements.sync"
    }));

    let first_lease = host
        .lease_ready_jobs(
            &plan.jobs.topology.domain_events_queue,
            "worker-a",
            JobInstant::from_unix_seconds(200),
            Duration::from_secs(30),
            1,
        )
        .unwrap()
        .remove(0);
    let retry = host
        .acknowledge_failed(
            &first_lease,
            JobInstant::from_unix_seconds(205),
            DeadLetterReason::PolicyViolation,
            "temporary membership projection failure",
        )
        .unwrap();

    assert!(matches!(
        retry,
        JobFailureDisposition::Retried { ref queue, .. }
            if queue == &plan.jobs.topology.domain_events_queue
    ));
}

#[test]
fn jobs_host_requires_scheduler_leadership_for_scheduled_jobs() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .build()
        .unwrap();
    let mut host = plan.jobs_host("scheduler-a").unwrap();
    let scheduled =
        JobDispatchRequest::new("cms.publish-scheduled", "publish scheduled landing page")
            .unwrap()
            .scheduled_for(JobInstant::from_unix_seconds(120))
            .with_idempotency_key("cms.publish-scheduled:page-42")
            .unwrap();

    let job_id = host
        .enqueue_job(scheduled, JobInstant::from_unix_seconds(100))
        .unwrap();
    assert_eq!(host.coordinator().scheduled_jobs().len(), 1);

    let err = host
        .promote_due_jobs(JobInstant::from_unix_seconds(120))
        .unwrap_err();
    assert!(matches!(
        err,
        RuntimeJobsError::Jobs(JobsModelError::MissingSchedulerLeadership { node_id })
            if node_id == "scheduler-a"
    ));

    host.acquire_scheduler_leadership(JobInstant::from_unix_seconds(110), Duration::from_secs(60))
        .unwrap();
    let promoted = host
        .promote_due_jobs(JobInstant::from_unix_seconds(120))
        .unwrap();
    assert_eq!(promoted, vec![job_id]);
    assert_eq!(host.coordinator().ready_jobs().len(), 1);
}

#[test]
fn jobs_host_rejects_domain_event_jobs_without_event_dispatch() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(MembershipsModule::new())
        .build()
        .unwrap();
    let mut host = plan.jobs_host("scheduler-a").unwrap();
    let request = JobDispatchRequest::new(
        "memberships.entitlements.sync",
        "attempt to bypass event flow",
    )
    .unwrap()
    .with_idempotency_key("memberships.entitlements.sync:42")
    .unwrap();

    let err = host
        .enqueue_job(request, JobInstant::from_unix_seconds(50))
        .unwrap_err();
    assert_eq!(
        err,
        RuntimeJobsError::DomainEventJobRequiresEventDispatch {
            job: "memberships.entitlements.sync".to_string(),
        }
    );
}

#[test]
fn runtime_builder_rejects_duplicate_runtime_job_names() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let first = StaticManifestModule::new(ModuleManifest::new("invalid").with_jobs(vec![
        JobContract::new(
            "shared.job",
            JobTriggerKind::Operator,
            true,
            "first copy of a duplicated runtime job",
        ),
    ]));
    let second =
        StaticManifestModule::new(
            ModuleManifest::new("other").with_jobs(vec![JobContract::new(
                "shared.job",
                JobTriggerKind::Webhook,
                true,
                "second copy of a duplicated runtime job",
            )]),
        );

    let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(first)
        .with_module(second)
        .build()
        .unwrap_err();

    match error {
        RuntimeBuildError::DuplicateRuntimeJobName {
            job,
            first_module,
            second_module,
        } => {
            assert_eq!(job, "shared.job");
            assert_eq!(first_module, "invalid");
            assert_eq!(second_module, "other");
        }
        other => panic!("expected duplicate runtime job error, got {other:?}"),
    }
}

#[test]
fn ops_host_queues_report_exports_into_the_jobs_runtime() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(MembershipsModule::new())
        .with_module(OpsModule::new())
        .build()
        .unwrap();
    let mut ops = plan.ops_host("scheduler-a").unwrap();

    let queued = ops
        .queue_report_export(
            ReportExportRequest::new(
                ReportExportId::new("export-memberships-1").unwrap(),
                ReportId::new("report.memberships.summary").unwrap(),
                "operator-1",
                JobInstant::from_unix_seconds(100),
            )
            .unwrap()
            .with_capability(Capability::MembershipSubscriptionManage)
            .with_idempotency_key(IdempotencyKey::new("report:memberships:summary:1").unwrap()),
        )
        .unwrap();

    assert_eq!(
        queued.plan.definition.id.as_str(),
        "report.memberships.summary"
    );
    assert_eq!(queued.queued_job_id.as_str(), "export-memberships-1");
    assert_eq!(ops.jobs().coordinator().ready_jobs().len(), 1);
    assert_eq!(
        ops.jobs().coordinator().ready_jobs()[0]
            .spec
            .job_name
            .as_str(),
        "report.export.report.memberships.summary"
    );
}

#[test]
fn ops_host_enforces_bulk_capabilities_before_queueing_jobs() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(EventsModule::new())
        .with_module(OpsModule::new())
        .build()
        .unwrap();
    let mut ops = plan.ops_host("scheduler-a").unwrap();

    let denied = ops
        .queue_bulk_operation(
            BulkOperationRequest::new(
                BulkExecutionId::new("bulk-check-in-1").unwrap(),
                BulkOperationId::new("bulk.events.check-in").unwrap(),
                "operator-1",
                JobInstant::from_unix_seconds(100),
                25,
            )
            .unwrap()
            .with_idempotency_key(IdempotencyKey::new("bulk:events:check-in:1").unwrap()),
        )
        .unwrap_err();
    assert!(matches!(
        denied,
        RuntimeOpsError::Ops(OpsModelError::MissingCapability {
            operation: "bulk operation",
            required: Capability::EventsBookingCheckIn,
        })
    ));

    let queued = ops
        .queue_bulk_operation(
            BulkOperationRequest::new(
                BulkExecutionId::new("bulk-check-in-2").unwrap(),
                BulkOperationId::new("bulk.events.check-in").unwrap(),
                "operator-1",
                JobInstant::from_unix_seconds(110),
                25,
            )
            .unwrap()
            .with_capability(Capability::EventsBookingCheckIn)
            .with_idempotency_key(IdempotencyKey::new("bulk:events:check-in:2").unwrap()),
        )
        .unwrap();

    assert_eq!(queued.queued_job_id.as_str(), "bulk-check-in-2");
    assert_eq!(ops.jobs().coordinator().ready_jobs().len(), 1);
    assert_eq!(
        ops.jobs().coordinator().ready_jobs()[0]
            .spec
            .job_name
            .as_str(),
        "bulk.bulk.events.check-in"
    );
}

#[test]
fn runtime_plan_creates_tls_host_with_expected_provider_mode() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let host = plan.tls_host().unwrap();
    let status = host.status();

    assert_eq!(status.customer_app, "showcase-events");
    assert_eq!(status.mode, davenda_config::TlsMode::Acme);
    assert_eq!(status.edge_mode, EdgeMode::DirectTermination);
    assert_eq!(
        status.provider,
        Some(CertificateProviderKind::CloudflareDns)
    );
    assert!(status.inventory.certificates().is_empty());
}

#[test]
fn tls_host_status_tracks_control_plane_inventory_renewals_and_pending_challenges() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let mut host = plan.tls_host().unwrap();
    let certificate_id = CertificateId::new("cert-live").unwrap();

    host.import_certificate(active_certificate("cert-live", "www.example.com"))
        .unwrap();
    host.queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();
    host.begin_renewal(&certificate_id, CertificateId::new("cert-next").unwrap())
        .unwrap();

    let status = host.status();
    assert_eq!(status.inventory.certificates().len(), 1);
    assert_eq!(status.queued_renewals.len(), 1);
    assert_eq!(status.pending_challenges.len(), 1);
    assert_eq!(
        status.pending_challenges[0]
            .replacement_certificate_id
            .as_ref()
            .map(|id| id.as_str()),
        Some("cert-next")
    );
}

#[test]
fn cloned_tls_control_plane_handles_share_state_within_the_runtime_facade() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let host = plan.tls_host().unwrap();
    let control_plane = host.control_plane().clone();
    let mirrored_control_plane = control_plane.clone();
    let certificate_id = CertificateId::new("cert-control-plane").unwrap();

    control_plane
        .import_certificate(active_certificate(
            "cert-control-plane",
            "control-plane.example.com",
        ))
        .unwrap();
    mirrored_control_plane
        .queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();

    assert_eq!(
        control_plane
            .inventory()
            .record(&certificate_id)
            .unwrap()
            .status,
        CertificateStatus::RenewalDue
    );
    assert_eq!(mirrored_control_plane.renewal_queue().len(), 1);
}

#[test]
fn tls_host_activate_replacement_emits_control_plane_hot_reload_and_supersedes_old_certificate() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let mut host = plan.tls_host().unwrap();
    let certificate_id = CertificateId::new("cert-live").unwrap();

    host.import_certificate(active_certificate("cert-live", "shop.example.com"))
        .unwrap();
    host.queue_renewal(&certificate_id, TlsInstant::from_unix_seconds(3_900_000))
        .unwrap();
    host.begin_renewal(&certificate_id, CertificateId::new("cert-next").unwrap())
        .unwrap();

    let event = host
        .activate_replacement(
            &certificate_id,
            active_certificate("cert-next", "shop.example.com")
                .with_cloudflare_mode(CloudflareEncryptionMode::FullStrict),
        )
        .unwrap();
    assert_eq!(event.certificate_id.as_str(), "cert-next");
    assert!(event.reloaded_without_restart);

    let status = host.status();
    assert_eq!(status.hot_reload_events.len(), 1);
    assert_eq!(
        status
            .inventory
            .active_for_hostname(&Hostname::new("shop.example.com").unwrap())
            .unwrap()
            .id
            .as_str(),
        "cert-next"
    );
    assert_eq!(
        status.inventory.record(&certificate_id).unwrap().status,
        CertificateStatus::Superseded
    );
}

#[test]
fn tls_host_rejects_external_termination_issuance_and_preserves_cloudflare_origin_mode() {
    let external = PlatformConfig::from_toml_str(&external_tls_config()).unwrap();
    let external_plan = RuntimeBuilder::new(external, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let external_host = external_plan.tls_host().unwrap();

    let err = external_host
        .issue_for_bindings(vec![HostnameBinding::new(
            Hostname::new("www.example.com").unwrap(),
            CustomerAppId::new("showcase-events").unwrap(),
        )])
        .unwrap_err();
    assert_eq!(
        err,
        RuntimeTlsError::Tls(TlsModelError::ExternalTerminationDoesNotIssue)
    );

    let origin = PlatformConfig::from_toml_str(&cloudflare_origin_tls_config()).unwrap();
    let origin_plan = RuntimeBuilder::new(origin, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let issue_plan = origin_plan
        .tls_host()
        .unwrap()
        .issue_for_bindings(vec![HostnameBinding::new(
            Hostname::new("origin.example.com").unwrap(),
            CustomerAppId::new("showcase-events").unwrap(),
        )])
        .unwrap();

    assert_eq!(
        issue_plan.cloudflare_mode,
        Some(CloudflareEncryptionMode::FullStrict)
    );
}
