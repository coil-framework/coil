use super::core::CmsModule;
use coil_core::{ModuleManifest, PlatformModule, RegistrationError, ServiceRegistry};
use coil_data::MigrationPlan;

mod manifest;
mod registration;

use manifest::build_manifest;
use registration::register_module_services;

impl PlatformModule for CmsModule {
    fn manifest(&self) -> ModuleManifest {
        build_manifest(self)
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        register_module_services(self, registry)
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        Some(CmsModule::migration_plan(self).expect("cms migration plan is constant and valid"))
    }
}
