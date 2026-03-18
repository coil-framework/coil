use std::collections::HashMap;
use std::time::Duration;

use davenda_auth::{AuthModelPackage, Capability};
use davenda_cache::{CachePlanner, CacheTopology, DistributedCacheBackend};
use davenda_config::{DistributedCache, PlatformConfig, TlsMode};
use davenda_wasm::{ExtensionPointKind, ResourceLimits};
use thiserror::Error;

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

#[derive(Debug, Clone)]
pub struct CoreBootstrap {
    pub registry: ServiceRegistry,
    pub cache: CacheRuntimeServices,
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

    registry.register_core_service("core.storage", "Storage policy and object access layer")?;
    registry.register_core_service("core.assets", "Asset manifest and CDN publication layer")?;
    registry.register_core_service("core.template", "HTML-first template runtime")?;
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
    use davenda_wasm::ExtensionPointKind;

    const VALID_CONFIG: &str = r#"
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

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
        assert!(ids.contains(&"core.wasm"));
        assert!(ids.contains(&"core.wasm.limits"));
        assert_eq!(
            bootstrap.cache.distributed_backend(),
            Some(DistributedCacheBackend::Redis)
        );
        assert!(bootstrap.cache.shared_invalidation_enabled());
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
}

fn cache_topology_from_config(config: &PlatformConfig) -> CacheTopology {
    match config.cache.l2 {
        Some(DistributedCache::Redis) => CacheTopology::with_redis(),
        Some(DistributedCache::Valkey) => CacheTopology::with_valkey(),
        None => CacheTopology::moka_only(),
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
