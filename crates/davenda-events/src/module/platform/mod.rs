use super::*;

mod manifest;
mod migrations;
mod registration;

use manifest::build_manifest;
use migrations::install_module_migration_plan;
use registration::register_module_services;

impl PlatformModule for EventsModule {
    fn manifest(&self) -> ModuleManifest {
        build_manifest(self)
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        register_module_services(self, registry)
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        install_module_migration_plan(self)
    }
}
