use davenda_core::{PlatformModule, RegistrationError, ServiceRegistry};
use davenda_data::MigrationPlan;

use crate::AdminShell;

mod manifest;
mod migrations;
mod registration;
mod shell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminModule {
    name: String,
    config_namespace: String,
    shell: AdminShell,
}

impl AdminModule {
    pub fn new() -> Self {
        Self {
            name: "admin".to_string(),
            config_namespace: "admin".to_string(),
            shell: shell::default_shell(),
        }
    }

    pub fn shell(&self) -> &AdminShell {
        &self.shell
    }
}

impl Default for AdminModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for AdminModule {
    fn manifest(&self) -> davenda_core::ModuleManifest {
        manifest::build_manifest(self)
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registration::register_services(self, registry)
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        migrations::build_migration_plan(self)
    }
}
