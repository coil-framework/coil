use std::collections::HashMap;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use davenda_auth::{AuthModelPackage, Capability};
use davenda_cache::{CachePlanner, CacheTopology, DistributedCacheBackend};
use davenda_config::{
    CookieConfig as HttpCookieConfig, CsrfConfig as HttpCsrfConfig, DistributedCache,
    PlatformConfig, SameSitePolicy, SessionStore as ConfigSessionStore, TlsMode,
};
use davenda_template::{TemplateNamespace, TemplateRegistry, TemplateRuntime};
use davenda_wasm::{ExtensionPointKind, ResourceLimits};
use hmac::{Hmac, Mac};
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDescriptor {
    pub id: String,
    pub owner: ServiceOwner,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceOwner {
    Core,
    Module(String),
    CustomerApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheRuntimeServices {
    pub topology: CacheTopology,
    pub planner: CachePlanner,
}

impl CacheRuntimeServices {
    pub fn shared_invalidation_enabled(&self) -> bool {
        self.topology.supports_shared_invalidation()
    }

    pub fn distributed_backend(&self) -> Option<DistributedCacheBackend> {
        self.topology.l2()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmRuntimeServices {
    pub extension_directory: String,
    pub allow_network: bool,
    pub limits: WasmLimitsProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmLimitsProfile {
    pub page: ResourceLimits,
    pub api: ResourceLimits,
    pub job: ResourceLimits,
    pub scheduled_job: ResourceLimits,
    pub webhook: ResourceLimits,
    pub admin_widget: ResourceLimits,
    pub render_hook: ResourceLimits,
}

impl WasmLimitsProfile {
    pub fn for_point(&self, point: ExtensionPointKind) -> ResourceLimits {
        match point {
            ExtensionPointKind::Page => self.page,
            ExtensionPointKind::Api => self.api,
            ExtensionPointKind::Job => self.job,
            ExtensionPointKind::ScheduledJob => self.scheduled_job,
            ExtensionPointKind::Webhook => self.webhook,
            ExtensionPointKind::AdminWidget => self.admin_widget,
            ExtensionPointKind::RenderHook => self.render_hook,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionStoreTopology {
    Memory,
    Database,
    Redis,
    Valkey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieProtection {
    Signed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookiePolicy {
    pub name: String,
    pub domain: Option<String>,
    pub path: String,
    pub same_site: SameSitePolicy,
    pub secure: bool,
    pub http_only: bool,
    pub protection: CookieProtection,
}

impl CookiePolicy {
    pub fn from_config(config: &HttpCookieConfig, protection: CookieProtection) -> Self {
        Self {
            name: config.name.clone(),
            domain: config.domain.clone(),
            path: config.path.clone(),
            same_site: config.same_site,
            secure: config.secure,
            http_only: config.http_only,
            protection,
        }
    }

    pub fn render_set_cookie(&self, value: &str, max_age: Option<Duration>) -> String {
        let mut attributes = vec![format!("{}={value}", self.name)];
        attributes.push(format!("Path={}", self.path));

        if let Some(domain) = &self.domain {
            attributes.push(format!("Domain={domain}"));
        }

        if let Some(max_age) = max_age {
            attributes.push(format!("Max-Age={}", max_age.as_secs()));
        }

        attributes.push(format!(
            "SameSite={}",
            match self.same_site {
                SameSitePolicy::Lax => "Lax",
                SameSitePolicy::Strict => "Strict",
                SameSitePolicy::None => "None",
            }
        ));

        if self.secure {
            attributes.push("Secure".to_string());
        }

        if self.http_only {
            attributes.push("HttpOnly".to_string());
        }

        attributes.join("; ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieSigner {
    pub policy: CookiePolicy,
}

impl CookieSigner {
    pub fn new(policy: CookiePolicy) -> Self {
        Self { policy }
    }

    pub fn sign(&self, secret: &[u8], value: &str) -> Result<String, BrowserSecurityError> {
        let payload = URL_SAFE_NO_PAD.encode(value.as_bytes());
        let signature = sign_payload(secret, payload.as_bytes())?;
        Ok(format!("v1.{payload}.{signature}"))
    }

    pub fn verify(&self, secret: &[u8], encoded: &str) -> Result<String, BrowserSecurityError> {
        let mut parts = encoded.split('.');
        let version = parts.next();
        let payload = parts.next();
        let signature = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidCookieFormat);
        }

        let payload = payload.ok_or(BrowserSecurityError::InvalidCookieFormat)?;
        let signature = signature.ok_or(BrowserSecurityError::InvalidCookieFormat)?;
        verify_payload(secret, payload.as_bytes(), signature)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| BrowserSecurityError::InvalidCookieFormat)?;

        String::from_utf8(bytes).map_err(|_| BrowserSecurityError::InvalidCookieFormat)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSecurityServices {
    pub store: SessionStoreTopology,
    pub idle_timeout: Duration,
    pub absolute_timeout: Duration,
    pub session_cookie: CookiePolicy,
    pub flash_cookie: CookiePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsrfProtection {
    pub enabled: bool,
    pub field_name: String,
    pub header_name: String,
}

impl CsrfProtection {
    pub fn from_config(config: &HttpCsrfConfig) -> Self {
        Self {
            enabled: config.enabled,
            field_name: config.field_name.clone(),
            header_name: config.header_name.clone(),
        }
    }

    pub fn issue_token(
        &self,
        secret: &[u8],
        session_id: &str,
        action: &str,
    ) -> Result<String, BrowserSecurityError> {
        if !self.enabled {
            return Err(BrowserSecurityError::CsrfDisabled);
        }

        let mut nonce = [0u8; 16];
        OsRng.fill_bytes(&mut nonce);
        let nonce = URL_SAFE_NO_PAD.encode(nonce);
        let payload = format!("{session_id}:{action}:{nonce}");
        let signature = sign_payload(secret, payload.as_bytes())?;
        Ok(format!("v1.{nonce}.{signature}"))
    }

    pub fn verify_token(
        &self,
        secret: &[u8],
        session_id: &str,
        action: &str,
        token: &str,
    ) -> Result<bool, BrowserSecurityError> {
        if !self.enabled {
            return Ok(true);
        }

        let mut parts = token.split('.');
        let version = parts.next();
        let nonce = parts.next();
        let signature = parts.next();

        if version != Some("v1") || parts.next().is_some() {
            return Err(BrowserSecurityError::InvalidCsrfTokenFormat);
        }

        let nonce = nonce.ok_or(BrowserSecurityError::InvalidCsrfTokenFormat)?;
        let signature = signature.ok_or(BrowserSecurityError::InvalidCsrfTokenFormat)?;
        let payload = format!("{session_id}:{action}:{nonce}");

        Ok(verify_payload(secret, payload.as_bytes(), signature).is_ok())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSecurityServices {
    pub sessions: SessionSecurityServices,
    pub csrf: CsrfProtection,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrowserSecurityError {
    #[error("browser security operations require a non-empty secret")]
    EmptySecret,
    #[error("cookie value is not in the expected signed format")]
    InvalidCookieFormat,
    #[error("signed cookie failed verification")]
    InvalidCookieSignature,
    #[error("CSRF protection is disabled for this runtime")]
    CsrfDisabled,
    #[error("CSRF token is not in the expected format")]
    InvalidCsrfTokenFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRuntimeServices {
    pub customer_app_namespace: TemplateNamespace,
    pub core_namespace: TemplateNamespace,
    pub registry: TemplateRegistry,
    pub runtime: TemplateRuntime,
}

impl TemplateRuntimeServices {
    pub fn namespace_chain(
        &self,
        module_namespace: Option<&TemplateNamespace>,
    ) -> Vec<TemplateNamespace> {
        let mut chain = vec![self.customer_app_namespace.clone()];

        if let Some(module_namespace) = module_namespace {
            if module_namespace != &self.customer_app_namespace
                && module_namespace != &self.core_namespace
            {
                chain.push(module_namespace.clone());
            }
        }

        chain.push(self.core_namespace.clone());
        chain
    }
}

#[derive(Debug, Clone)]
pub struct CoreBootstrap {
    pub registry: ServiceRegistry,
    pub cache: CacheRuntimeServices,
    pub browser: BrowserSecurityServices,
    pub template: TemplateRuntimeServices,
    pub wasm: WasmRuntimeServices,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleManifest {
    pub name: String,
    pub required_capabilities: Vec<Capability>,
    pub optional_capabilities: Vec<Capability>,
    pub config_namespace: Option<String>,
}

impl ModuleManifest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            required_capabilities: Vec::new(),
            optional_capabilities: Vec::new(),
            config_namespace: None,
        }
    }

    pub fn with_required_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.required_capabilities = capabilities;
        self
    }

    pub fn with_optional_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.optional_capabilities = capabilities;
        self
    }

    pub fn with_config_namespace(mut self, config_namespace: impl Into<String>) -> Self {
        self.config_namespace = Some(config_namespace.into());
        self
    }
}

pub trait PlatformModule {
    fn manifest(&self) -> ModuleManifest;
    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError>;
}

#[derive(Debug, Default, Clone)]
pub struct ServiceRegistry {
    services: HashMap<String, ServiceDescriptor>,
    modules: HashMap<String, ModuleManifest>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_core_service(
        &mut self,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceDescriptor {
            id: id.into(),
            owner: ServiceOwner::Core,
            description: description.into(),
        })
    }

    pub fn register_module_service(
        &mut self,
        module: impl Into<String>,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceDescriptor {
            id: id.into(),
            owner: ServiceOwner::Module(module.into()),
            description: description.into(),
        })
    }

    pub fn register_customer_app_service(
        &mut self,
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceDescriptor {
            id: id.into(),
            owner: ServiceOwner::CustomerApp,
            description: description.into(),
        })
    }

    pub fn register_module_manifest(
        &mut self,
        manifest: ModuleManifest,
    ) -> Result<(), RegistrationError> {
        if self.modules.contains_key(&manifest.name) {
            return Err(RegistrationError::DuplicateModule {
                name: manifest.name.clone(),
            });
        }

        self.modules.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    pub fn services(&self) -> impl Iterator<Item = &ServiceDescriptor> {
        self.services.values()
    }

    pub fn modules(&self) -> impl Iterator<Item = &ModuleManifest> {
        self.modules.values()
    }

    fn register(&mut self, service: ServiceDescriptor) -> Result<(), RegistrationError> {
        if self.services.contains_key(&service.id) {
            return Err(RegistrationError::DuplicateService {
                id: service.id.clone(),
            });
        }

        self.services.insert(service.id.clone(), service);
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistrationError {
    #[error("service `{id}` is already registered")]
    DuplicateService { id: String },
    #[error("module `{name}` is already registered")]
    DuplicateModule { name: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityValidationError {
    #[error(
        "module `{module}` requires capability `{capability}` but the active auth package does not bind it"
    )]
    MissingCapability {
        module: String,
        capability: Capability,
    },
}

pub fn bootstrap_core_services(
    config: &PlatformConfig,
) -> Result<CoreBootstrap, RegistrationError> {
    let mut registry = ServiceRegistry::new();
    let cache_topology = cache_topology_from_config(config);
    let cache = CacheRuntimeServices {
        topology: cache_topology,
        planner: CachePlanner::new(cache_topology),
    };
    let browser = browser_security_from_config(config);
    let template = template_runtime_services();
    let wasm = wasm_runtime_from_config(config);

    registry.register_core_service("core.config", "Typed platform configuration")?;
    registry.register_core_service("core.logging", "Structured logging service")?;

    if config.observability.tracing {
        registry.register_core_service("core.tracing", "Distributed tracing pipeline")?;
    }

    registry.register_core_service("core.auth", "Authorization engine and model loader")?;
    registry.register_core_service(
        "core.cache.l1",
        format!("Local cache backend: {}", cache.topology.l1()),
    )?;

    if let Some(distributed) = cache.distributed_backend() {
        registry.register_core_service(
            "core.cache.l2",
            format!("Distributed cache backend: {distributed}"),
        )?;
        registry.register_core_service(
            "core.cache.invalidation",
            format!("Shared invalidation, coalescing, and coordination via {distributed}"),
        )?;
    }
    registry.register_core_service(
        "core.cache.http",
        "HTTP cache-control, validators, variation keys, and surrogate tags",
    )?;
    registry.register_core_service(
        "core.http",
        "HTTP request pipeline, middleware ordering, and typed request context",
    )?;
    registry.register_core_service(
        "core.http.sessions",
        format!(
            "Server-side session policy backed by {:?}",
            browser.sessions.store
        ),
    )?;
    registry.register_core_service(
        "core.http.cookies",
        "Signed cookie policy with central Secure, HttpOnly, SameSite, domain, and path defaults",
    )?;
    registry.register_core_service(
        "core.http.csrf",
        "CSRF token issuance and validation for state-changing browser flows",
    )?;

    registry.register_core_service("core.storage", "Storage policy and object access layer")?;
    registry.register_core_service("core.assets", "Asset manifest and CDN publication layer")?;
    registry.register_core_service("core.template", "HTML-first template runtime")?;
    registry.register_core_service(
        "core.template.fragments",
        "Named fragment, slot, and partial-rendering composition runtime",
    )?;
    registry.register_core_service(
        "core.wasm",
        format!(
            "WASM extension host runtime rooted at `{}` with network {}",
            wasm.extension_directory,
            if wasm.allow_network {
                "enabled"
            } else {
                "disabled"
            }
        ),
    )?;
    registry.register_core_service(
        "core.wasm.limits",
        "Per-surface WASM resource limits for pages, APIs, jobs, webhooks, and widgets",
    )?;
    registry.register_core_service("core.jobs", "Background jobs and scheduler")?;

    match config.tls.mode {
        TlsMode::External => {
            registry.register_core_service(
                "core.tls.metadata",
                "Trusted termination metadata and secure transport policy",
            )?;
        }
        _ => {
            registry.register_core_service(
                "core.tls",
                "Certificate lifecycle, TLS termination, and renewal orchestration",
            )?;
        }
    }

    Ok(CoreBootstrap {
        registry,
        cache,
        browser,
        template,
        wasm,
    })
}

pub fn validate_module_capabilities<P>(
    package: &P,
    manifest: &ModuleManifest,
) -> Result<(), CapabilityValidationError>
where
    P: AuthModelPackage,
{
    for capability in &manifest.required_capabilities {
        if package.binding_for(*capability).is_none() {
            return Err(CapabilityValidationError::MissingCapability {
                module: manifest.name.clone(),
                capability: *capability,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_cache::DistributedCacheBackend;
    use davenda_config::PlatformConfig;
    use davenda_template::TemplateNamespace;
    use davenda_wasm::ExtensionPointKind;

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
object_store = "s3"
local_root = "/var/lib/platform"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false

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

    #[test]
    fn bootstrap_registers_core_services() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let bootstrap = bootstrap_core_services(&config).unwrap();

        let ids = bootstrap
            .registry
            .services()
            .map(|service| service.id.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"core.config"));
        assert!(ids.contains(&"core.auth"));
        assert!(ids.contains(&"core.tls"));
        assert!(ids.contains(&"core.cache.l1"));
        assert!(ids.contains(&"core.cache.l2"));
        assert!(ids.contains(&"core.cache.invalidation"));
        assert!(ids.contains(&"core.cache.http"));
        assert!(ids.contains(&"core.http"));
        assert!(ids.contains(&"core.http.sessions"));
        assert!(ids.contains(&"core.http.cookies"));
        assert!(ids.contains(&"core.http.csrf"));
        assert!(ids.contains(&"core.template.fragments"));
        assert!(ids.contains(&"core.wasm"));
        assert!(ids.contains(&"core.wasm.limits"));
        assert_eq!(
            bootstrap.cache.distributed_backend(),
            Some(DistributedCacheBackend::Redis)
        );
        assert!(bootstrap.cache.shared_invalidation_enabled());
        assert_eq!(
            bootstrap.browser.sessions.store,
            SessionStoreTopology::Redis
        );
        assert_eq!(
            bootstrap.browser.sessions.idle_timeout,
            Duration::from_secs(3600)
        );
        assert_eq!(
            bootstrap.browser.sessions.session_cookie.name,
            "davenda_session"
        );
        assert_eq!(bootstrap.browser.csrf.field_name, "_csrf");
        assert_eq!(
            bootstrap
                .template
                .namespace_chain(Some(&TemplateNamespace::new("events").unwrap())),
            vec![
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateNamespace::new("events").unwrap(),
                TemplateNamespace::new("core").unwrap(),
            ]
        );
        assert_eq!(bootstrap.wasm.extension_directory, "extensions");
        assert!(!bootstrap.wasm.allow_network);
        assert_eq!(
            bootstrap
                .wasm
                .limits
                .for_point(ExtensionPointKind::Page)
                .max_runtime,
            Duration::from_millis(50)
        );
        assert_eq!(
            bootstrap
                .wasm
                .limits
                .for_point(ExtensionPointKind::Job)
                .max_runtime,
            Duration::from_secs(30)
        );
    }

    #[test]
    fn validates_module_capabilities_against_auth_package() {
        let package = DefaultAuthModelPackage::default();
        let manifest = ModuleManifest::new("cms-pages")
            .with_required_capabilities(vec![Capability::CmsPageRead, Capability::CmsPagePublish]);

        assert!(validate_module_capabilities(&package, &manifest).is_ok());
    }

    #[test]
    fn signed_cookie_round_trips_and_rejects_tampering() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let bootstrap = bootstrap_core_services(&config).unwrap();
        let signer = CookieSigner::new(bootstrap.browser.sessions.session_cookie.clone());
        let secret = b"0123456789abcdef0123456789abcdef";

        let signed = signer.sign(secret, "sess_123").unwrap();
        assert_eq!(signer.verify(secret, &signed).unwrap(), "sess_123");

        let mut tampered = signed.clone();
        let last = tampered.pop().unwrap();
        tampered.push(if last == 'A' { 'B' } else { 'A' });
        assert!(signer.verify(secret, &tampered).is_err());
    }

    #[test]
    fn csrf_tokens_bind_to_session_and_action() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let bootstrap = bootstrap_core_services(&config).unwrap();
        let secret = b"abcdef0123456789abcdef0123456789";

        let token = bootstrap
            .browser
            .csrf
            .issue_token(secret, "sess_123", "/checkout")
            .unwrap();

        assert!(
            bootstrap
                .browser
                .csrf
                .verify_token(secret, "sess_123", "/checkout", &token)
                .unwrap()
        );
        assert!(
            !bootstrap
                .browser
                .csrf
                .verify_token(secret, "sess_999", "/checkout", &token)
                .unwrap()
        );
    }
}

fn cache_topology_from_config(config: &PlatformConfig) -> CacheTopology {
    match config.cache.l2 {
        Some(DistributedCache::Redis) => CacheTopology::with_redis(),
        Some(DistributedCache::Valkey) => CacheTopology::with_valkey(),
        None => CacheTopology::moka_only(),
    }
}

fn browser_security_from_config(config: &PlatformConfig) -> BrowserSecurityServices {
    BrowserSecurityServices {
        sessions: SessionSecurityServices {
            store: match config.http.session.store {
                ConfigSessionStore::Memory => SessionStoreTopology::Memory,
                ConfigSessionStore::Database => SessionStoreTopology::Database,
                ConfigSessionStore::Redis => SessionStoreTopology::Redis,
                ConfigSessionStore::Valkey => SessionStoreTopology::Valkey,
            },
            idle_timeout: Duration::from_secs(config.http.session.idle_timeout_secs),
            absolute_timeout: Duration::from_secs(config.http.session.absolute_timeout_secs),
            session_cookie: CookiePolicy::from_config(
                &config.http.session_cookie,
                CookieProtection::Signed,
            ),
            flash_cookie: CookiePolicy::from_config(
                &config.http.flash_cookie,
                CookieProtection::Signed,
            ),
        },
        csrf: CsrfProtection::from_config(&config.http.csrf),
    }
}

fn template_runtime_services() -> TemplateRuntimeServices {
    let registry = TemplateRegistry::new();

    TemplateRuntimeServices {
        customer_app_namespace: TemplateNamespace::new("customer-app")
            .expect("constant template namespace is valid"),
        core_namespace: TemplateNamespace::new("core")
            .expect("constant template namespace is valid"),
        runtime: TemplateRuntime::new(registry.clone()),
        registry,
    }
}

fn wasm_runtime_from_config(config: &PlatformConfig) -> WasmRuntimeServices {
    let request_limit = Duration::from_millis(config.wasm.default_time_limit_ms);
    let tighten = |point| tighten_runtime_limit(ResourceLimits::baseline_for(point), request_limit);

    WasmRuntimeServices {
        extension_directory: config.wasm.directory.clone(),
        allow_network: config.wasm.allow_network,
        limits: WasmLimitsProfile {
            page: tighten(ExtensionPointKind::Page),
            api: tighten(ExtensionPointKind::Api),
            admin_widget: tighten(ExtensionPointKind::AdminWidget),
            render_hook: tighten(ExtensionPointKind::RenderHook),
            webhook: tighten(ExtensionPointKind::Webhook),
            job: ResourceLimits::baseline_for(ExtensionPointKind::Job),
            scheduled_job: ResourceLimits::baseline_for(ExtensionPointKind::ScheduledJob),
        },
    }
}

fn tighten_runtime_limit(mut limits: ResourceLimits, max_runtime: Duration) -> ResourceLimits {
    if max_runtime < limits.max_runtime {
        limits.max_runtime = max_runtime;
    }

    limits
}

fn sign_payload(secret: &[u8], payload: &[u8]) -> Result<String, BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn verify_payload(
    secret: &[u8],
    payload: &[u8],
    signature: &str,
) -> Result<(), BrowserSecurityError> {
    if secret.is_empty() {
        return Err(BrowserSecurityError::EmptySecret);
    }

    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| BrowserSecurityError::InvalidCookieSignature)?;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts arbitrary key lengths");
    mac.update(payload);
    mac.verify_slice(&signature)
        .map_err(|_| BrowserSecurityError::InvalidCookieSignature)
}
