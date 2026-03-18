use std::collections::BTreeMap;

use davenda_config::{ConfigError, PlatformConfig};
use davenda_core::{
    CoreService, ModuleDescriptor, ModuleId, PlatformModule, RegistrationContext,
    RegistrationError, ServiceRegistry,
};
use thiserror::Error;

pub struct PlatformBuilder {
    config: PlatformConfig,
    modules: Vec<Box<dyn PlatformModule>>,
}

impl PlatformBuilder {
    pub fn new(config: PlatformConfig) -> Self {
        Self {
            config,
            modules: Vec::new(),
        }
    }

    pub fn register_module<M>(mut self, module: M) -> Self
    where
        M: PlatformModule + 'static,
    {
        self.modules.push(Box::new(module));
        self
    }

    pub fn build(self) -> Result<Platform, RuntimeError> {
        self.config.validate()?;

        let mut registry = ServiceRegistry::default();
        register_core_services(&self.config, &mut registry)?;

        let mut available = BTreeMap::new();
        for module in self.modules {
            let descriptor = module.descriptor();
            if available
                .insert(
                    descriptor.id.clone(),
                    RegisteredModule { descriptor, module },
                )
                .is_some()
            {
                return Err(RuntimeError::DuplicateModule(
                    available
                        .keys()
                        .last()
                        .expect("duplicate insert implies a key exists")
                        .as_str()
                        .into(),
                ));
            }
        }

        let enabled_modules = self
            .config
            .modules
            .enabled
            .iter()
            .map(|name| ModuleId::new(name.clone()))
            .collect::<Vec<_>>();

        for enabled in &enabled_modules {
            if !available.contains_key(enabled) {
                return Err(RuntimeError::MissingModule(enabled.as_str().into()));
            }
        }

        for enabled in &enabled_modules {
            let registered = available
                .get(enabled)
                .expect("enabled module existence checked above");

            for dependency in &registered.descriptor.dependencies {
                if !enabled_modules.iter().any(|module| module == dependency) {
                    return Err(RuntimeError::MissingDependency {
                        module: enabled.as_str().into(),
                        dependency: dependency.as_str().into(),
                    });
                }
            }

            for service in &registered.descriptor.required_core_services {
                let key = davenda_core::ServiceKey::core(service.as_str());
                if !registry.contains(&key) {
                    return Err(RuntimeError::MissingCoreService {
                        module: enabled.as_str().into(),
                        service: service.as_str().into(),
                    });
                }
            }
        }

        let mut descriptors = BTreeMap::new();
        let mut modules = BTreeMap::new();

        for enabled in enabled_modules {
            let registered = available
                .remove(&enabled)
                .expect("enabled module existence checked above");

            let mut context =
                RegistrationContext::new(&self.config, &registered.descriptor.id, &mut registry);
            registered.module.register(&mut context)?;

            descriptors.insert(
                registered.descriptor.id.as_str().to_owned(),
                registered.descriptor.clone(),
            );
            modules.insert(registered.descriptor.id.as_str().to_owned(), registered);
        }

        Ok(Platform {
            config: self.config,
            services: registry,
            modules: descriptors,
        })
    }
}

#[derive(Debug)]
pub struct Platform {
    config: PlatformConfig,
    services: ServiceRegistry,
    modules: BTreeMap<String, ModuleDescriptor>,
}

impl Platform {
    pub fn config(&self) -> &PlatformConfig {
        &self.config
    }

    pub fn services(&self) -> &ServiceRegistry {
        &self.services
    }

    pub fn modules(&self) -> &BTreeMap<String, ModuleDescriptor> {
        &self.modules
    }
}

struct RegisteredModule {
    descriptor: ModuleDescriptor,
    module: Box<dyn PlatformModule>,
}

fn register_core_services(
    config: &PlatformConfig,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_core(CoreService::Config, "typed platform configuration")?;
    registry.register_core(CoreService::Cache, "cache policy and adapters")?;
    registry.register_core(CoreService::Storage, "storage drivers and policy engine")?;
    registry.register_core(
        CoreService::Assets,
        "asset manifest and publication services",
    )?;
    registry.register_core(
        CoreService::Auth,
        "authorization engine and capability checks",
    )?;
    registry.register_core(CoreService::Wasm, "WASM host and extension runtime")?;
    registry.register_core(CoreService::Jobs, "job scheduler and workers")?;
    registry.register_core(
        CoreService::Seo,
        "SEO metadata and structured data services",
    )?;
    registry.register_core(CoreService::I18n, "locale and translation services")?;
    registry.register_core(CoreService::A11y, "accessibility-aware rendering contracts")?;

    if config.observability.logs {
        registry.register_core(CoreService::Logging, "structured application logging")?;
    }
    if config.observability.metrics {
        registry.register_core(
            CoreService::Metrics,
            "metrics and cardinality-safe telemetry",
        )?;
    }
    if config.observability.tracing {
        registry.register_core(CoreService::Tracing, "distributed tracing context")?;
    }
    if config.observability.health_endpoint {
        registry.register_core(CoreService::Health, "health and readiness reporting")?;
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Registration(#[from] RegistrationError),
    #[error("module `{0}` is enabled in config but not registered with the builder")]
    MissingModule(String),
    #[error("module `{module}` requires `{dependency}`, but that dependency is not enabled")]
    MissingDependency { module: String, dependency: String },
    #[error("module `{module}` requires core service `{service}`, but it was not registered")]
    MissingCoreService { module: String, service: String },
    #[error("module `{0}` was registered more than once")]
    DuplicateModule(String),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use davenda_config::ModulesConfig;
    use davenda_core::{ModuleId, ServiceKey};

    use super::*;

    #[derive(Clone)]
    struct FakeModule {
        descriptor: ModuleDescriptor,
        service_name: &'static str,
    }

    impl FakeModule {
        fn new(id: &str, service_name: &'static str) -> Self {
            Self {
                descriptor: ModuleDescriptor::new(id),
                service_name,
            }
        }
    }

    impl PlatformModule for FakeModule {
        fn descriptor(&self) -> ModuleDescriptor {
            self.descriptor.clone()
        }

        fn register(&self, context: &mut RegistrationContext<'_>) -> Result<(), RegistrationError> {
            context.register_service(self.service_name, "module-owned service")
        }
    }

    #[test]
    fn builder_registers_core_services_and_enabled_modules() {
        let config = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["cms-pages".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let platform = PlatformBuilder::new(config)
            .register_module(FakeModule::new("cms-pages", "routes"))
            .build()
            .unwrap();

        assert!(
            platform
                .services()
                .contains(&ServiceKey::core(CoreService::Config.as_str()))
        );
        assert!(
            platform
                .services()
                .contains(&ServiceKey::module(&ModuleId::new("cms-pages"), "routes"))
        );
        assert!(platform.modules().contains_key("cms-pages"));
    }

    #[test]
    fn builder_requires_enabled_modules_to_be_registered() {
        let config = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["events".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let error = PlatformBuilder::new(config).build().unwrap_err();
        assert!(matches!(error, RuntimeError::MissingModule(module) if module == "events"));
    }

    #[test]
    fn builder_checks_declared_module_dependencies() {
        let config = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["events".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let mut events = FakeModule::new("events", "routes");
        events
            .descriptor
            .dependencies
            .insert(ModuleId::new("memberships"));

        let error = PlatformBuilder::new(config)
            .register_module(events)
            .build()
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::MissingDependency { module, dependency }
                if module == "events" && dependency == "memberships"
        ));
    }

    #[test]
    fn builder_checks_required_core_services() {
        let config = PlatformConfig {
            observability: davenda_config::ObservabilityConfig {
                logs: false,
                metrics: false,
                tracing: false,
                health_endpoint: false,
            },
            modules: ModulesConfig {
                enabled: vec!["metrics-consumer".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let mut module = FakeModule::new("metrics-consumer", "routes");
        module
            .descriptor
            .required_core_services
            .insert(CoreService::Metrics);

        let error = PlatformBuilder::new(config)
            .register_module(module)
            .build()
            .unwrap_err();

        assert!(matches!(
            error,
            RuntimeError::MissingCoreService { module, service }
                if module == "metrics-consumer" && service == "metrics"
        ));
    }

    #[test]
    fn duplicate_module_registration_is_rejected() {
        let config = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["cms-pages".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let error = PlatformBuilder::new(config)
            .register_module(FakeModule::new("cms-pages", "routes"))
            .register_module(FakeModule::new("cms-pages", "other-routes"))
            .build()
            .unwrap_err();

        assert!(matches!(error, RuntimeError::DuplicateModule(module) if module == "cms-pages"));
    }

    #[test]
    fn descriptor_supports_capabilities_and_dependencies() {
        let mut descriptor = ModuleDescriptor::new("events");
        descriptor.dependencies = BTreeSet::from([ModuleId::new("memberships")]);
        descriptor.required_core_services = BTreeSet::from([CoreService::Auth, CoreService::Jobs]);
        descriptor.capability_contracts =
            BTreeSet::from(["events.booking.manage".into(), "events.booking.read".into()]);

        assert_eq!(descriptor.dependencies.len(), 1);
        assert_eq!(descriptor.required_core_services.len(), 2);
        assert!(
            descriptor
                .capability_contracts
                .contains("events.booking.manage")
        );
    }
}
