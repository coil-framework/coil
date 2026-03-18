use std::collections::{BTreeMap, BTreeSet, VecDeque};

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

        let ordered_modules = resolve_module_order(&enabled_modules, &available)?;
        let mut descriptors = BTreeMap::new();

        for enabled in ordered_modules {
            let registered = available
                .remove(&enabled)
                .expect("enabled module existence checked above");
            let config_namespace = registered
                .descriptor
                .config_namespace
                .as_deref()
                .unwrap_or_else(|| registered.descriptor.id.as_str());

            let mut context = RegistrationContext::new(
                &self.config,
                &registered.descriptor.id,
                config_namespace,
                &mut registry,
            );
            registered.module.register(&mut context)?;

            descriptors.insert(
                registered.descriptor.id.as_str().to_owned(),
                registered.descriptor.clone(),
            );
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

fn resolve_module_order(
    enabled_modules: &[ModuleId],
    available: &BTreeMap<ModuleId, RegisteredModule>,
) -> Result<Vec<ModuleId>, RuntimeError> {
    let enabled_set = enabled_modules.iter().cloned().collect::<BTreeSet<_>>();
    let mut indegree = enabled_modules
        .iter()
        .cloned()
        .map(|module| (module, 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut outgoing = enabled_modules
        .iter()
        .cloned()
        .map(|module| (module, Vec::<ModuleId>::new()))
        .collect::<BTreeMap<_, _>>();

    for module in enabled_modules {
        let registered = available
            .get(module)
            .expect("enabled module existence checked before ordering");

        for dependency in &registered.descriptor.dependencies {
            if enabled_set.contains(dependency) {
                *indegree
                    .get_mut(module)
                    .expect("indegree is pre-seeded for all enabled modules") += 1;
                outgoing
                    .get_mut(dependency)
                    .expect("outgoing edges are pre-seeded for all enabled modules")
                    .push(module.clone());
            }
        }
    }

    let mut queue = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(module, _)| module.clone())
        .collect::<Vec<_>>();
    queue.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut queue = VecDeque::from(queue);
    let mut ordered = Vec::with_capacity(enabled_modules.len());

    while let Some(module) = queue.pop_front() {
        ordered.push(module.clone());

        if let Some(dependents) = outgoing.get(&module) {
            let mut dependents = dependents.clone();
            dependents.sort_by(|left, right| left.as_str().cmp(right.as_str()));

            for dependent in dependents {
                let remaining = indegree
                    .get_mut(&dependent)
                    .expect("indegree is pre-seeded for all enabled modules");
                *remaining -= 1;
                if *remaining == 0 {
                    queue.push_back(dependent);
                }
            }
        }
    }

    if ordered.len() != enabled_modules.len() {
        let unresolved = indegree
            .into_iter()
            .filter(|(_, degree)| *degree > 0)
            .map(|(module, _)| module.as_str().to_owned())
            .collect::<Vec<_>>();
        return Err(RuntimeError::CircularDependency(unresolved));
    }

    Ok(ordered)
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
    #[error("enabled modules contain a circular dependency: {0:?}")]
    CircularDependency(Vec<String>),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex};

    use davenda_config::ModulesConfig;
    use davenda_core::{ModuleId, ServiceKey};

    use super::*;

    #[derive(Clone)]
    struct FakeModule {
        descriptor: ModuleDescriptor,
        service_name: &'static str,
        registration_log: Option<Arc<Mutex<Vec<String>>>>,
        captured_settings: Option<Arc<Mutex<Vec<String>>>>,
    }

    impl FakeModule {
        fn new(id: &str, service_name: &'static str) -> Self {
            Self {
                descriptor: ModuleDescriptor::new(id),
                service_name,
                registration_log: None,
                captured_settings: None,
            }
        }
    }

    impl PlatformModule for FakeModule {
        fn descriptor(&self) -> ModuleDescriptor {
            self.descriptor.clone()
        }

        fn register(&self, context: &mut RegistrationContext<'_>) -> Result<(), RegistrationError> {
            if let Some(log) = &self.registration_log {
                log.lock()
                    .expect("registration log should not be poisoned")
                    .push(self.descriptor.id.as_str().to_owned());
            }
            if let Some(settings_log) = &self.captured_settings {
                let snapshot = context
                    .module_settings()
                    .and_then(|value| value.get("label"))
                    .and_then(|value| value.as_str())
                    .unwrap_or("missing")
                    .to_owned();
                settings_log
                    .lock()
                    .expect("settings log should not be poisoned")
                    .push(snapshot);
            }
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

    #[test]
    fn modules_register_in_dependency_order() {
        let order = Arc::new(Mutex::new(Vec::new()));
        let config = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["events".into(), "memberships".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let mut memberships = FakeModule::new("memberships", "services");
        memberships.registration_log = Some(order.clone());

        let mut events = FakeModule::new("events", "routes");
        events
            .descriptor
            .dependencies
            .insert(ModuleId::new("memberships"));
        events.registration_log = Some(order.clone());

        PlatformBuilder::new(config)
            .register_module(events)
            .register_module(memberships)
            .build()
            .unwrap();

        let order = order.lock().unwrap().clone();
        assert_eq!(order, vec!["memberships".to_string(), "events".to_string()]);
    }

    #[test]
    fn circular_module_dependencies_are_rejected() {
        let config = PlatformConfig {
            modules: ModulesConfig {
                enabled: vec!["events".into(), "memberships".into()],
                settings: Default::default(),
            },
            ..PlatformConfig::default()
        };

        let mut memberships = FakeModule::new("memberships", "services");
        memberships
            .descriptor
            .dependencies
            .insert(ModuleId::new("events"));

        let mut events = FakeModule::new("events", "routes");
        events
            .descriptor
            .dependencies
            .insert(ModuleId::new("memberships"));

        let error = PlatformBuilder::new(config)
            .register_module(events)
            .register_module(memberships)
            .build()
            .unwrap_err();

        assert!(matches!(error, RuntimeError::CircularDependency(_)));
    }

    #[test]
    fn registration_context_exposes_module_settings() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let config = PlatformConfig::from_toml_str(
            r#"
                [app]
                name = "settings-app"

                [modules]
                enabled = ["events"]

                [modules.settings.events]
                label = "configured"
            "#,
        )
        .unwrap();

        let mut events = FakeModule::new("events", "routes");
        events.captured_settings = Some(captured.clone());

        PlatformBuilder::new(config)
            .register_module(events)
            .build()
            .unwrap();

        assert_eq!(captured.lock().unwrap().as_slice(), &["configured"]);
    }
}
