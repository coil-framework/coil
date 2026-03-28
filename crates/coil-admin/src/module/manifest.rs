use coil_auth::Capability;
use coil_core::{
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind, IntegrationPoint,
    JobContract, JobTriggerKind, MigrationContract, ModuleBehavior, ModuleManifest, RouteSurface,
    RouteSurfaceKind,
};

use super::AdminModule;

pub(super) fn build_manifest(module: &AdminModule) -> ModuleManifest {
    ModuleManifest::new(module.name.clone())
        .with_required_capabilities(vec![
            Capability::AdminShellAccess,
            Capability::AdminAuditRead,
        ])
        .with_optional_capabilities(vec![
            Capability::SystemModuleManage,
            Capability::SystemConfigRead,
            Capability::SystemConfigWrite,
        ])
        .with_config_namespace(module.config_namespace.clone())
        .with_capability_contracts(vec![
            CapabilityContract::required(Capability::AdminShellAccess, ["admin_module"]),
            CapabilityContract::required(Capability::AdminAuditRead, ["audit_entry"]),
            CapabilityContract::optional(Capability::SystemModuleManage, ["admin_module"]),
            CapabilityContract::optional(Capability::SystemConfigRead, ["admin_module"]),
            CapabilityContract::optional(Capability::SystemConfigWrite, ["admin_module"]),
        ])
        .with_core_service_dependencies(vec![
            CoreServiceDependency::Auth,
            CoreServiceDependency::A11y,
            CoreServiceDependency::Template,
            CoreServiceDependency::I18n,
            CoreServiceDependency::Observability,
            CoreServiceDependency::Http,
        ])
        .with_migrations(vec![MigrationContract::new(
            "admin.audit_log",
            10,
            "Creates audit-log storage for operator actions and workflow traces",
        )])
        .with_route_surfaces(vec![
            RouteSurface::new("admin.dashboard", RouteSurfaceKind::AdminPage, "/admin")
                .gated_by(Capability::AdminShellAccess),
            RouteSurface::new("admin.audit", RouteSurfaceKind::AdminPage, "/admin/audit")
                .gated_by(Capability::AdminAuditRead),
        ])
        .with_jobs(vec![JobContract::new(
            "admin.audit.export",
            JobTriggerKind::Operator,
            true,
            "Exports audit history and operator traces without blocking request handling",
        )])
        .with_event_subscriptions(vec![EventSubscription::new(
            "system.audit.entry-recorded",
            Some("admin.audit.export"),
            "Allows downstream audit export and retention workflows to react to recorded operator actions",
        )])
        .with_integration_points(vec![
            IntegrationPoint::new(
                IntegrationKind::AdminNavigation,
                "admin.shell",
                "Provides the shared back-office frame, navigation, and operator session entry points",
            ),
            IntegrationPoint::new(
                IntegrationKind::AdminWorkflow,
                "admin.audit",
                "Centralizes audit visibility for actions performed by official modules and customer apps",
            ),
        ])
        .with_behaviors(vec![
            ModuleBehavior::AccessibleAdminUi,
            ModuleBehavior::AuditedBulkActions,
        ])
        .with_extension_slots(vec![ExtensionSlotDescriptor::new(
            ExtensionSlotKind::AdminWidget,
            "admin.dashboard.summary",
            "Allows bounded customer widgets to participate in the shared admin dashboard",
        )])
        .with_http_surfaces(vec![
            HttpSurfaceContribution::page(
                "admin.dashboard",
                HttpSurfaceArea::Admin,
                "/admin",
                "admin/dashboard",
            )
            .gated_by(Capability::AdminShellAccess),
            HttpSurfaceContribution::page(
                "admin.audit",
                HttpSurfaceArea::Admin,
                "/admin/audit",
                "admin/audit",
            )
            .gated_by(Capability::AdminAuditRead),
        ])
}
