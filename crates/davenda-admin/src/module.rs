use davenda_auth::Capability;
use davenda_core::{
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind, IntegrationPoint,
    JobContract, JobTriggerKind, MigrationContract, ModuleBehavior, ModuleManifest, PlatformModule,
    RegistrationError, RouteSurface, RouteSurfaceKind, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

use crate::{
    AccessibilityContract, AdminResourceDescriptor, AdminResourceId, AdminResourceKind, AdminShell,
    AdminWidgetDescriptor, AdminWidgetId, BulkActionKind, NavigationSection, WidgetSlot,
    WorkflowAction, WorkflowId,
};

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
            shell: AdminShell::new(
                AccessibilityContract::standard(),
                vec![
                    AdminResourceDescriptor::new(
                        AdminResourceId::new("admin.dashboard").expect("valid id"),
                        "/admin",
                        "Dashboard",
                        "Dashboard",
                        NavigationSection::Overview,
                        AdminResourceKind::Dashboard,
                        Capability::AdminShellAccess,
                    )
                    .expect("constant resource is valid"),
                    AdminResourceDescriptor::new(
                        AdminResourceId::new("admin.audit").expect("valid id"),
                        "/admin/audit",
                        "Audit Log",
                        "Audit",
                        NavigationSection::System,
                        AdminResourceKind::Audit,
                        Capability::AdminAuditRead,
                    )
                    .expect("constant resource is valid"),
                    AdminResourceDescriptor::new(
                        AdminResourceId::new("admin.modules").expect("valid id"),
                        "/admin/system/modules",
                        "Modules",
                        "Modules",
                        NavigationSection::System,
                        AdminResourceKind::Settings,
                        Capability::SystemModuleManage,
                    )
                    .expect("constant resource is valid"),
                ],
                vec![
                    AdminWidgetDescriptor::new(
                        AdminWidgetId::new("admin.status").expect("valid id"),
                        "Platform status",
                        WidgetSlot::Header,
                        Some(Capability::AdminShellAccess),
                        Some("/admin".to_string()),
                    )
                    .expect("constant widget is valid"),
                    AdminWidgetDescriptor::new(
                        AdminWidgetId::new("admin.audit.summary").expect("valid id"),
                        "Recent privileged actions",
                        WidgetSlot::Sidebar,
                        Some(Capability::AdminAuditRead),
                        Some("/admin/audit".to_string()),
                    )
                    .expect("constant widget is valid"),
                ],
                vec![WorkflowAction::new(
                    WorkflowId::new("system.modules.apply").expect("valid id"),
                    "Apply module changes",
                    BulkActionKind::Custom,
                    Capability::SystemModuleManage,
                    "Module changes scheduled",
                )
                .expect("constant workflow is valid")],
            )
            .expect("constant shell is valid"),
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
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::AdminAuditRead,
            ])
            .with_optional_capabilities(vec![
                Capability::SystemModuleManage,
                Capability::SystemConfigRead,
                Capability::SystemConfigWrite,
            ])
            .with_config_namespace(self.config_namespace.clone())
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

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.admin.shell",
            "Shared admin shell, routing frame, and operator layout",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.admin.navigation",
            "Capability-gated admin navigation, sections, and resource visibility",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.admin.widgets",
            "Dashboard and page widgets constrained by shell-defined slots",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.admin.workflows",
            "Bulk actions, workflow plans, and operator task surfaces",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.admin.audit",
            "Audit log access and privileged action attribution",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.admin.accessibility",
            "Accessibility-aware admin interaction contracts for forms, tables, and focus",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("admin_audit_log").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create admin audit storage for operator actions",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS admin_audit_log (id TEXT PRIMARY KEY, actor_id TEXT NOT NULL, action TEXT NOT NULL, created_at BIGINT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("admin migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("admin_dashboard_state")
                    .expect("constant migration id is valid"),
                owner,
                20,
                "Create dashboard state storage for admin shell preferences",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS admin_dashboard_state (operator_id TEXT PRIMARY KEY, layout_json TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("admin migration ids are unique");
        Some(plan)
    }
}
