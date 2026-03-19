use davenda_auth::Capability;

use crate::{
    AccessibilityContract, AdminResourceDescriptor, AdminResourceId, AdminResourceKind,
    AdminShell, AdminWidgetDescriptor, AdminWidgetId, BulkActionKind, NavigationSection,
    WidgetSlot, WorkflowAction, WorkflowId,
};

pub(super) fn default_shell() -> AdminShell {
    AdminShell::new(
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
        vec![
            WorkflowAction::new(
                WorkflowId::new("system.modules.apply").expect("valid id"),
                "Apply module changes",
                BulkActionKind::Custom,
                Capability::SystemModuleManage,
                "Module changes scheduled",
            )
            .expect("constant workflow is valid"),
        ],
    )
    .expect("constant shell is valid")
}
