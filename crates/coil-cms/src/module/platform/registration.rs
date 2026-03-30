use super::super::core::CmsModule;
use coil_core::{RegistrationError, ServiceRegistry};

pub(super) fn register_module_services(
    module: &CmsModule,
    registry: &mut ServiceRegistry,
) -> Result<(), RegistrationError> {
    registry.register_module_service(
        module.name.clone(),
        "module.cms.pages",
        "CMS page definitions, revisions, page settings, and publication workflow",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.cms.page_builder",
        "CMS structured block schemas, page block instances, and shared editorial content",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.cms.navigation",
        "CMS navigation trees and localized route composition",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.cms.redirects",
        "CMS redirects and route handoff rules",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.cms.admin",
        "CMS admin resources, editorial workflow screens, and previews",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.cms.media_refs",
        "CMS media references bound to managed assets and publication state",
    )?;
    registry.register_module_service(
        module.name.clone(),
        "module.cms.shared_blocks",
        "CMS shared block inventory and reusable editorial fragments",
    )
}
