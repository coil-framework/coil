use super::*;

pub(super) fn register_module_services(
    module: &OpsModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name().to_string(),
        "module.ops.search",
        "Declarative search indexing contributions, visibility rules, and rebuild metadata",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.ops.reports",
        "Asynchronous report definitions, export planning, and delivery policies",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.ops.bulk",
        "Capability-gated bulk operations with audit-ready, idempotent job planning",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.ops.jobs",
        "Jobs-backed execution planning for reports and bulk workflows",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.ops.audit",
        "Operator visibility into search, reporting, and bulk action plans",
    )
}
