use coil_core::{RegistrationError, ServiceRegistry};

use super::AdminModule;

pub(super) fn register_services(
    module: &AdminModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name.clone(),
        "module.admin.shell",
        "Shared admin shell, routing frame, and operator layout",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.admin.navigation",
        "Capability-gated admin navigation, sections, and resource visibility",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.admin.widgets",
        "Dashboard and page widgets constrained by shell-defined slots",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.admin.workflows",
        "Bulk actions, workflow plans, and operator task surfaces",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.admin.audit",
        "Audit log access and privileged action attribution",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.admin.accessibility",
        "Accessibility-aware admin interaction contracts for forms, tables, and focus",
    )
}
