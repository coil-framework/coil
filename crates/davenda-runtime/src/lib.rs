use davenda_auth::AuthModelPackage;
use davenda_config::{ConfigError, PlatformConfig};
use davenda_core::{
    CapabilityValidationError, ModuleManifest, PlatformModule, RegistrationError,
    ServiceDescriptor, bootstrap_core_services, validate_module_capabilities,
};
use thiserror::Error;

pub struct RuntimeBuilder<P> {
    config: PlatformConfig,
    auth_package: P,
    modules: Vec<Box<dyn PlatformModule>>,
}

impl<P> RuntimeBuilder<P>
where
    P: AuthModelPackage,
{
    pub fn new(config: PlatformConfig, auth_package: P) -> Self {
        Self {
            config,
            auth_package,
            modules: Vec::new(),
        }
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn build(self) -> Result<RuntimePlan, RuntimeBuildError> {
        self.config.validate().map_err(ConfigError::Validation)?;

        if self.auth_package.manifest().name != self.config.auth.package {
            return Err(RuntimeBuildError::AuthPackageMismatch {
                configured: self.config.auth.package,
                actual: self.auth_package.manifest().name.clone(),
            });
        }

        let mut registry = bootstrap_core_services(&self.config)?;
        let mut module_manifests = Vec::new();

        for module in self.modules {
            let manifest = module.manifest();
            validate_module_capabilities(&self.auth_package, &manifest)?;
            registry.register_module_manifest(manifest.clone())?;
            module.register(&mut registry)?;
            module_manifests.push(manifest);
        }

        Ok(RuntimePlan {
            config: self.config,
            auth_package_name: self.auth_package.manifest().name.clone(),
            services: registry.services().cloned().collect(),
            modules: module_manifests,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RuntimePlan {
    pub config: PlatformConfig,
    pub auth_package_name: String,
    pub services: Vec<ServiceDescriptor>,
    pub modules: Vec<ModuleManifest>,
}

#[derive(Debug, Error)]
pub enum RuntimeBuildError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error(transparent)]
    Capability(#[from] CapabilityValidationError),
    #[error("configured auth package `{configured}` does not match loaded package `{actual}`")]
    AuthPackageMismatch { configured: String, actual: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_auth::{Capability, DefaultAuthModelPackage};
    use davenda_core::{ModuleManifest, PlatformModule, RegistrationError, ServiceRegistry};

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

    struct CmsPagesModule;

    impl PlatformModule for CmsPagesModule {
        fn manifest(&self) -> ModuleManifest {
            ModuleManifest::new("cms-pages").with_required_capabilities(vec![
                Capability::CmsPageRead,
                Capability::CmsPagePublish,
            ])
        }

        fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
            registry.register_module_service(
                "cms-pages",
                "module.cms.pages",
                "CMS page routes and content services",
            )
        }
    }

    #[test]
    fn runtime_builder_creates_a_runtime_plan() {
        let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .with_module(CmsPagesModule)
            .build()
            .unwrap();

        assert_eq!(plan.auth_package_name, "platform-default-auth");
        assert!(
            plan.services
                .iter()
                .any(|service| service.id == "module.cms.pages")
        );
        assert_eq!(plan.modules.len(), 1);
        assert_eq!(plan.modules[0].name, "cms-pages");
    }
}
