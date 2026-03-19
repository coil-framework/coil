use super::super::support::events_waitlist_repository;
use super::*;

#[path = "backoffice.rs"]
mod backoffice;
#[path = "capabilities.rs"]
mod capabilities;
#[path = "operations.rs"]
mod operations;
#[path = "surfaces.rs"]
mod surfaces;

use backoffice::{bulk_operations, report_definitions, search_contributions};
use capabilities::{
    capability_contracts, core_service_dependencies, module_dependencies,
    optional_capabilities, required_capabilities,
};
use operations::{
    event_subscriptions, extension_slots, jobs, module_behaviors, module_migrations,
};
use surfaces::{http_surfaces, integration_points, route_surfaces};

pub(super) fn build_manifest(module: &EventsModule) -> ModuleManifest {
    ModuleManifest::new(module.name.clone())
        .with_required_capabilities(required_capabilities())
        .with_optional_capabilities(optional_capabilities())
        .with_config_namespace(module.config_namespace.clone())
        .with_capability_contracts(capability_contracts())
        .with_module_dependencies(module_dependencies())
        .with_core_service_dependencies(core_service_dependencies())
        .with_migrations(module_migrations())
        .with_route_surfaces(route_surfaces())
        .with_jobs(jobs())
        .with_event_subscriptions(event_subscriptions())
        .with_integration_points(integration_points())
        .with_behaviors(module_behaviors())
        .with_extension_slots(extension_slots())
        .with_admin_resources(module.admin_resources.clone())
        .with_search_contributions(search_contributions())
        .with_report_definitions(report_definitions())
        .with_bulk_operations(bulk_operations())
        .with_data_repositories(vec![events_waitlist_repository()])
        .with_http_surfaces(http_surfaces())
}
