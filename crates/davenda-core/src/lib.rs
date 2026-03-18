use std::collections::{BTreeMap, BTreeSet};

use davenda_config::PlatformConfig;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleId(String);

impl ModuleId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ModuleId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ModuleId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceKey(String);

impl ServiceKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn core(name: &str) -> Self {
        Self::new(format!("core.{name}"))
    }

    pub fn module(module: &ModuleId, name: &str) -> Self {
        Self::new(format!("module.{}.{}", module.as_str(), name))
    }

    pub fn app(name: &str) -> Self {
        Self::new(format!("app.{name}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreService {
    Config,
    Logging,
    Metrics,
    Tracing,
    Health,
    Cache,
    Storage,
    Assets,
    Auth,
    Wasm,
    Jobs,
    Seo,
    I18n,
    A11y,
}

impl CoreService {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Logging => "logging",
            Self::Metrics => "metrics",
            Self::Tracing => "tracing",
            Self::Health => "health",
            Self::Cache => "cache",
            Self::Storage => "storage",
            Self::Assets => "assets",
            Self::Auth => "auth",
            Self::Wasm => "wasm",
            Self::Jobs => "jobs",
            Self::Seo => "seo",
            Self::I18n => "i18n",
            Self::A11y => "a11y",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDescriptor {
    pub key: ServiceKey,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct ServiceRegistry {
    services: BTreeMap<ServiceKey, ServiceDescriptor>,
}

impl ServiceRegistry {
    pub fn register(
        &mut self,
        key: ServiceKey,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        if self.services.contains_key(&key) {
            return Err(RegistrationError::DuplicateService(key.as_str().into()));
        }

        self.services.insert(
            key.clone(),
            ServiceDescriptor {
                key,
                description: description.into(),
            },
        );

        Ok(())
    }

    pub fn register_core(
        &mut self,
        service: CoreService,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.register(ServiceKey::core(service.as_str()), description)
    }

    pub fn contains(&self, key: &ServiceKey) -> bool {
        self.services.contains_key(key)
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &ServiceDescriptor> {
        self.services.values()
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDescriptor {
    pub id: ModuleId,
    pub dependencies: BTreeSet<ModuleId>,
    pub required_core_services: BTreeSet<CoreService>,
    pub capability_contracts: BTreeSet<String>,
    pub config_namespace: Option<String>,
}

impl ModuleDescriptor {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            dependencies: BTreeSet::new(),
            required_core_services: BTreeSet::new(),
            capability_contracts: BTreeSet::new(),
            config_namespace: None,
        }
    }
}

pub struct RegistrationContext<'a> {
    config: &'a PlatformConfig,
    module_id: &'a ModuleId,
    registry: &'a mut ServiceRegistry,
}

impl<'a> RegistrationContext<'a> {
    pub fn new(
        config: &'a PlatformConfig,
        module_id: &'a ModuleId,
        registry: &'a mut ServiceRegistry,
    ) -> Self {
        Self {
            config,
            module_id,
            registry,
        }
    }

    pub fn config(&self) -> &PlatformConfig {
        self.config
    }

    pub fn module_id(&self) -> &ModuleId {
        self.module_id
    }

    pub fn register_service(
        &mut self,
        name: &str,
        description: impl Into<String>,
    ) -> Result<(), RegistrationError> {
        self.registry
            .register(ServiceKey::module(self.module_id, name), description)
    }
}

pub trait PlatformModule: Send + Sync {
    fn descriptor(&self) -> ModuleDescriptor;

    fn register(&self, context: &mut RegistrationContext<'_>) -> Result<(), RegistrationError>;
}

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("service `{0}` is already registered")]
    DuplicateService(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_rejects_duplicate_service_keys() {
        let mut registry = ServiceRegistry::default();
        registry
            .register_core(CoreService::Config, "typed config")
            .unwrap();

        let error = registry
            .register_core(CoreService::Config, "another config")
            .unwrap_err();

        assert!(matches!(error, RegistrationError::DuplicateService(_)));
    }

    #[test]
    fn module_service_keys_are_namespaced() {
        let key = ServiceKey::module(&ModuleId::new("cms-pages"), "routes");
        assert_eq!(key.as_str(), "module.cms-pages.routes");
    }
}
