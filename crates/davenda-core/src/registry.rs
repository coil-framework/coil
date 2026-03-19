use super::*;

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

pub trait PlatformModule {
    fn manifest(&self) -> ModuleManifest;
    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError>;
    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        None
    }
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

