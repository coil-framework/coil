use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use davenda_auth::Capability;
use davenda_core::{
    CapabilityContract, CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor,
    ExtensionSlotKind, IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind,
    MigrationContract, ModuleBehavior, ModuleManifest, PlatformModule, RegistrationError,
    RouteSurface, RouteSurfaceKind, ServiceRegistry,
};
use davenda_data::{MigrationId, MigrationOwner, MigrationPlan, MigrationStep};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminModelError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    InvalidRoute { field: &'static str, value: String },
    DuplicateResource { resource_id: String },
    DuplicateWidget { widget_id: String },
    DuplicateWorkflow { workflow_id: String },
}

impl fmt::Display for AdminModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DuplicateResource { resource_id } => {
                write!(f, "admin resource `{resource_id}` is duplicated")
            }
            Self::DuplicateWidget { widget_id } => {
                write!(f, "admin widget `{widget_id}` is duplicated")
            }
            Self::DuplicateWorkflow { workflow_id } => {
                write!(f, "admin workflow `{workflow_id}` is duplicated")
            }
        }
    }
}

impl Error for AdminModelError {}

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, AdminModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(AdminResourceId, "admin_resource_id");
token_type!(AdminWidgetId, "admin_widget_id");
token_type!(WorkflowId, "workflow_id");
token_type!(AuditEntryId, "audit_entry_id");
token_type!(ResourceKind, "resource_kind");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdminResourceKind {
    Dashboard,
    ResourceIndex,
    DetailView,
    Workflow,
    Audit,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavigationSection {
    Overview,
    Content,
    Commerce,
    Memberships,
    Events,
    Media,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetSlot {
    Header,
    Summary,
    Sidebar,
    Footer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkActionKind {
    Publish,
    Refund,
    Cancel,
    CheckIn,
    Export,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessibilityContract {
    pub skip_link_target: String,
    pub live_region_id: String,
    pub error_summary_id: String,
    pub table_caption_required: bool,
    pub focus_restore_target: String,
}

impl AccessibilityContract {
    pub fn standard() -> Self {
        Self {
            skip_link_target: "admin-main".to_string(),
            live_region_id: "admin-status".to_string(),
            error_summary_id: "admin-errors".to_string(),
            table_caption_required: true,
            focus_restore_target: "page-title".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResourceDescriptor {
    pub id: AdminResourceId,
    pub route: String,
    pub title: String,
    pub nav_label: String,
    pub section: NavigationSection,
    pub kind: AdminResourceKind,
    pub required_capability: Capability,
}

impl AdminResourceDescriptor {
    pub fn new(
        id: AdminResourceId,
        route: impl Into<String>,
        title: impl Into<String>,
        nav_label: impl Into<String>,
        section: NavigationSection,
        kind: AdminResourceKind,
        required_capability: Capability,
    ) -> Result<Self, AdminModelError> {
        Ok(Self {
            id,
            route: validate_route("resource_route", route.into())?,
            title: require_non_empty("resource_title", title.into())?,
            nav_label: require_non_empty("resource_nav_label", nav_label.into())?,
            section,
            kind,
            required_capability,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminWidgetDescriptor {
    pub id: AdminWidgetId,
    pub title: String,
    pub slot: WidgetSlot,
    pub required_capability: Option<Capability>,
    pub resource_route: Option<String>,
}

impl AdminWidgetDescriptor {
    pub fn new(
        id: AdminWidgetId,
        title: impl Into<String>,
        slot: WidgetSlot,
        required_capability: Option<Capability>,
        resource_route: Option<String>,
    ) -> Result<Self, AdminModelError> {
        Ok(Self {
            id,
            title: require_non_empty("widget_title", title.into())?,
            slot,
            required_capability,
            resource_route: resource_route
                .map(|route| validate_route("widget_resource_route", route))
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAction {
    pub id: WorkflowId,
    pub title: String,
    pub bulk_action: BulkActionKind,
    pub required_capability: Capability,
    pub success_message: String,
}

impl WorkflowAction {
    pub fn new(
        id: WorkflowId,
        title: impl Into<String>,
        bulk_action: BulkActionKind,
        required_capability: Capability,
        success_message: impl Into<String>,
    ) -> Result<Self, AdminModelError> {
        Ok(Self {
            id,
            title: require_non_empty("workflow_title", title.into())?,
            bulk_action,
            required_capability,
            success_message: require_non_empty("workflow_success_message", success_message.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: AuditEntryId,
    pub actor_id: String,
    pub capability: Capability,
    pub resource_kind: ResourceKind,
    pub resource_id: String,
    pub action: String,
}

impl AuditEntry {
    pub fn new(
        id: AuditEntryId,
        actor_id: impl Into<String>,
        capability: Capability,
        resource_kind: ResourceKind,
        resource_id: impl Into<String>,
        action: impl Into<String>,
    ) -> Result<Self, AdminModelError> {
        Ok(Self {
            id,
            actor_id: require_non_empty("actor_id", actor_id.into())?,
            capability,
            resource_kind,
            resource_id: require_non_empty("resource_id", resource_id.into())?,
            action: require_non_empty("action", action.into())?,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorAccessContext {
    capabilities: HashSet<Capability>,
}

impl OperatorAccessContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    pub fn allows(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BulkActionPlan {
    pub workflow_id: WorkflowId,
    pub resource_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminShell {
    accessibility: AccessibilityContract,
    resources: Vec<AdminResourceDescriptor>,
    widgets: Vec<AdminWidgetDescriptor>,
    workflows: Vec<WorkflowAction>,
    audit_log: Vec<AuditEntry>,
}

impl AdminShell {
    pub fn new(
        accessibility: AccessibilityContract,
        resources: Vec<AdminResourceDescriptor>,
        widgets: Vec<AdminWidgetDescriptor>,
        workflows: Vec<WorkflowAction>,
    ) -> Result<Self, AdminModelError> {
        ensure_unique_resources(&resources)?;
        ensure_unique_widgets(&widgets)?;
        ensure_unique_workflows(&workflows)?;
        Ok(Self {
            accessibility,
            resources,
            widgets,
            workflows,
            audit_log: Vec::new(),
        })
    }

    pub fn accessibility(&self) -> &AccessibilityContract {
        &self.accessibility
    }

    pub fn visible_resources(
        &self,
        operator: &OperatorAccessContext,
    ) -> Vec<AdminResourceDescriptor> {
        self.resources
            .iter()
            .filter(|resource| operator.allows(resource.required_capability))
            .cloned()
            .collect()
    }

    pub fn navigation_by_section(
        &self,
        operator: &OperatorAccessContext,
    ) -> HashMap<NavigationSection, Vec<AdminResourceDescriptor>> {
        let mut grouped = HashMap::new();
        for resource in self.visible_resources(operator) {
            grouped
                .entry(resource.section)
                .or_insert_with(Vec::new)
                .push(resource);
        }
        grouped
    }

    pub fn visible_widgets(&self, operator: &OperatorAccessContext) -> Vec<AdminWidgetDescriptor> {
        self.widgets
            .iter()
            .filter(|widget| {
                widget
                    .required_capability
                    .is_none_or(|capability| operator.allows(capability))
            })
            .cloned()
            .collect()
    }

    pub fn build_bulk_action_plan(
        &self,
        workflow_id: &WorkflowId,
        resource_count: usize,
        operator: &OperatorAccessContext,
    ) -> Option<BulkActionPlan> {
        let workflow = self
            .workflows
            .iter()
            .find(|workflow| &workflow.id == workflow_id)?;
        if !operator.allows(workflow.required_capability) {
            return None;
        }

        Some(BulkActionPlan {
            workflow_id: workflow.id.clone(),
            resource_count,
            message: workflow.success_message.clone(),
        })
    }

    pub fn record_audit_entry(&mut self, entry: AuditEntry) {
        self.audit_log.push(entry);
    }

    pub fn visible_audit_entries(&self, operator: &OperatorAccessContext) -> &[AuditEntry] {
        if operator.allows(Capability::AdminAuditRead) {
            &self.audit_log
        } else {
            &[]
        }
    }
}

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
                vec![
                    WorkflowAction::new(
                        WorkflowId::new("system.modules.apply").expect("valid id"),
                        "Apply module changes",
                        BulkActionKind::Custom,
                        Capability::SystemModuleManage,
                        "Module changes scheduled",
                    )
                    .expect("constant workflow is valid"),
                    WorkflowAction::new(
                        WorkflowId::new("audit.export").expect("valid id"),
                        "Export audit log",
                        BulkActionKind::Export,
                        Capability::AdminAuditRead,
                        "Audit export queued",
                    )
                    .expect("constant workflow is valid"),
                ],
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
                CapabilityContract::required(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::required(
                    Capability::AdminAuditRead,
                    ["audit_entry"],
                ),
                CapabilityContract::optional(
                    Capability::SystemModuleManage,
                    ["admin_module"],
                ),
                CapabilityContract::optional(
                    Capability::SystemConfigRead,
                    ["admin_module"],
                ),
                CapabilityContract::optional(
                    Capability::SystemConfigWrite,
                    ["admin_module"],
                ),
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
                RouteSurface::new(
                    "admin.audit",
                    RouteSurfaceKind::AdminPage,
                    "/admin/audit",
                )
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

fn ensure_unique_resources(resources: &[AdminResourceDescriptor]) -> Result<(), AdminModelError> {
    let mut seen = HashSet::new();
    for resource in resources {
        if !seen.insert(resource.id.clone()) {
            return Err(AdminModelError::DuplicateResource {
                resource_id: resource.id.to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_unique_widgets(widgets: &[AdminWidgetDescriptor]) -> Result<(), AdminModelError> {
    let mut seen = HashSet::new();
    for widget in widgets {
        if !seen.insert(widget.id.clone()) {
            return Err(AdminModelError::DuplicateWidget {
                widget_id: widget.id.to_string(),
            });
        }
    }
    Ok(())
}

fn ensure_unique_workflows(workflows: &[WorkflowAction]) -> Result<(), AdminModelError> {
    let mut seen = HashSet::new();
    for workflow in workflows {
        if !seen.insert(workflow.id.clone()) {
            return Err(AdminModelError::DuplicateWorkflow {
                workflow_id: workflow.id.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_token(field: &'static str, value: String) -> Result<String, AdminModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AdminModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(AdminModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

fn validate_route(field: &'static str, value: String) -> Result<String, AdminModelError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(AdminModelError::InvalidRoute {
            field,
            value: route,
        })
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, AdminModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AdminModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_shell_filters_resources_and_widgets_by_capability() {
        let shell = AdminModule::new().shell().clone();
        let operator = OperatorAccessContext::new().with_capability(Capability::AdminShellAccess);

        let resources = shell.visible_resources(&operator);
        let widgets = shell.visible_widgets(&operator);

        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].route, "/admin");
        assert_eq!(widgets.len(), 1);
        assert_eq!(widgets[0].id, AdminWidgetId::new("admin.status").unwrap());
    }

    #[test]
    fn admin_shell_groups_navigation_and_plans_bulk_actions() {
        let shell = AdminModule::new().shell().clone();
        let operator = OperatorAccessContext::new()
            .with_capability(Capability::AdminAuditRead)
            .with_capability(Capability::SystemModuleManage);

        let grouped = shell.navigation_by_section(&operator);
        assert_eq!(grouped[&NavigationSection::System].len(), 2);

        let plan = shell
            .build_bulk_action_plan(&WorkflowId::new("audit.export").unwrap(), 42, &operator)
            .unwrap();
        assert_eq!(plan.resource_count, 42);
        assert_eq!(plan.message, "Audit export queued");
    }

    #[test]
    fn audit_visibility_requires_audit_capability() {
        let mut shell = AdminModule::new().shell().clone();
        shell.record_audit_entry(
            AuditEntry::new(
                AuditEntryId::new("audit-1").unwrap(),
                "user-1",
                Capability::CmsPagePublish,
                ResourceKind::new("page").unwrap(),
                "page-123",
                "published",
            )
            .unwrap(),
        );

        let denied = OperatorAccessContext::new().with_capability(Capability::AdminShellAccess);
        assert!(shell.visible_audit_entries(&denied).is_empty());

        let allowed = OperatorAccessContext::new().with_capability(Capability::AdminAuditRead);
        assert_eq!(shell.visible_audit_entries(&allowed).len(), 1);
    }

    #[test]
    fn admin_module_manifest_and_accessibility_contract_are_stable() {
        let module = AdminModule::new();
        let manifest = module.manifest();

        assert_eq!(manifest.name, "admin");
        assert_eq!(
            manifest.required_capabilities,
            vec![Capability::AdminShellAccess, Capability::AdminAuditRead]
        );
        assert!(
            manifest
                .optional_capabilities
                .contains(&Capability::SystemModuleManage)
        );
        assert_eq!(
            manifest.core_service_dependencies,
            vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::A11y,
                CoreServiceDependency::Template,
                CoreServiceDependency::I18n,
                CoreServiceDependency::Observability,
                CoreServiceDependency::Http,
            ]
        );
        assert_eq!(manifest.route_surfaces.len(), 2);
        assert_eq!(manifest.jobs.len(), 1);
        assert_eq!(manifest.event_subscriptions.len(), 1);
        assert!(
            manifest
                .extension_slots
                .iter()
                .any(|slot| slot.kind == ExtensionSlotKind::AdminWidget)
        );
        assert_eq!(
            module
                .install_migration_plan()
                .expect("admin migration plan")
                .ordered_steps()
                .len(),
            2
        );

        let accessibility = module.shell().accessibility();
        assert_eq!(accessibility.skip_link_target, "admin-main");
        assert_eq!(accessibility.live_region_id, "admin-status");
        assert!(accessibility.table_caption_required);
    }

    #[test]
    fn module_registration_exposes_admin_services() {
        let module = AdminModule::new();
        let mut registry = ServiceRegistry::new();
        module.register(&mut registry).unwrap();

        assert!(
            registry
                .services()
                .any(|service| service.id == "module.admin.shell")
        );
        assert!(
            registry
                .services()
                .any(|service| service.id == "module.admin.accessibility")
        );
    }
}
