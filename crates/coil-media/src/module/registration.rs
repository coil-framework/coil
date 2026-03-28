use super::*;

pub(super) fn register_module_services(
    module: &MediaModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name().to_string(),
        "module.media.libraries",
        "Media libraries, folder trees, and library-level policy defaults",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.media.folders",
        "Managed media folders and storage policy overrides",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.media.assets",
        "Managed media assets, revisions, publication state, and reuse across modules",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.media.metadata",
        "Metadata capture, derived metadata, and image handling",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.media.replacement",
        "Replacement workflows and revision promotion for managed assets",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.media.storage",
        "Storage policy interplay, delivery modes, and local-only overrides",
    )?;
    registry.register_module_service(
        module.name().to_string(),
        "module.media.admin",
        "Media admin resources and operator workflows",
    )
}
