use super::core::CommerceModule;
use coil_core::{PlatformModule, RegistrationError, ServiceRegistry};
use coil_data::MigrationPlan;

mod manifest;
mod registration;

use manifest::build_manifest;
use registration::register_module_services;

impl PlatformModule for CommerceModule {
    fn manifest(&self) -> coil_core::ModuleManifest {
        build_manifest(self)
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        register_module_services(self, registry)
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        Some(
            CommerceModule::migration_plan(self)
                .expect("commerce migration plan is constant and valid"),
        )
    }
}
