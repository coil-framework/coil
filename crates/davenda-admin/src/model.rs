use std::collections::{HashMap, HashSet};

use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind as CoreAdminContributionKind,
    AdminNavigationSection as CoreAdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition as CoreBulkOperationDefinition,
    BulkOperationKind as CoreBulkOperationKind, ModuleManifest,
};
use davenda_wasm::ExtensionRegistry;

use crate::error::AdminModelError;
use crate::ids::{AdminResourceId, AdminWidgetId, AuditEntryId, ResourceKind, WorkflowId};
use crate::validation::{require_non_empty, validate_route};

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

    pub fn from_contribution(
        contribution: &AdminResourceContribution,
    ) -> Result<Self, AdminModelError> {
        Self::new(
            AdminResourceId::new(contribution.id.clone())?,
            contribution.route.clone(),
            contribution.title.clone(),
            contribution.nav_label.clone(),
            contribution.section.into(),
            contribution.kind.into(),
            contribution.required_capability,
        )
    }
}

impl From<CoreAdminNavigationSection> for NavigationSection {
    fn from(value: CoreAdminNavigationSection) -> Self {
        match value {
            CoreAdminNavigationSection::Overview => Self::Overview,
            CoreAdminNavigationSection::Content => Self::Content,
            CoreAdminNavigationSection::Commerce => Self::Commerce,
            CoreAdminNavigationSection::Memberships => Self::Memberships,
            CoreAdminNavigationSection::Events => Self::Events,
            CoreAdminNavigationSection::Media => Self::Media,
            CoreAdminNavigationSection::System => Self::System,
        }
    }
}

impl From<CoreAdminContributionKind> for AdminResourceKind {
    fn from(value: CoreAdminContributionKind) -> Self {
        match value {
            CoreAdminContributionKind::Dashboard => Self::Dashboard,
            CoreAdminContributionKind::ResourceIndex => Self::ResourceIndex,
            CoreAdminContributionKind::DetailView => Self::DetailView,
            CoreAdminContributionKind::Workflow => Self::Workflow,
            CoreAdminContributionKind::Audit => Self::Audit,
            CoreAdminContributionKind::Settings => Self::Settings,
        }
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

    pub fn from_bulk_operation(
        definition: &CoreBulkOperationDefinition,
    ) -> Result<Self, AdminModelError> {
        Self::new(
            WorkflowId::new(definition.id.clone())?,
            definition.title.clone(),
            map_bulk_action_kind(definition.kind),
            definition.required_capability,
            format!("{} queued", definition.title),
        )
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

    pub fn resources(&self) -> &[AdminResourceDescriptor] {
        &self.resources
    }

    pub fn widgets(&self) -> &[AdminWidgetDescriptor] {
        &self.widgets
    }

    pub fn workflows(&self) -> &[WorkflowAction] {
        &self.workflows
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

    pub fn compose_module_resources(
        manifests: &[ModuleManifest],
    ) -> Result<Vec<AdminResourceDescriptor>, AdminModelError> {
        let mut resources = Vec::new();
        for manifest in manifests {
            for contribution in &manifest.admin_resources {
                resources.push(AdminResourceDescriptor::from_contribution(contribution)?);
            }
        }
        ensure_unique_resources(&resources)?;
        Ok(resources)
    }

    pub fn compose_module_workflows(
        manifests: &[ModuleManifest],
    ) -> Result<Vec<WorkflowAction>, AdminModelError> {
        let mut workflows = Vec::new();
        for manifest in manifests {
            for definition in &manifest.bulk_operations {
                workflows.push(WorkflowAction::from_bulk_operation(definition)?);
            }
        }
        ensure_unique_workflows(&workflows)?;
        Ok(workflows)
    }

    pub fn compose_extension_widgets(
        registry: &ExtensionRegistry,
    ) -> Result<Vec<AdminWidgetDescriptor>, AdminModelError> {
        let mut widgets = Vec::new();

        for handler in registry.registered_handlers() {
            if handler.point != davenda_wasm::ExtensionPointKind::AdminWidget {
                continue;
            }

            widgets.push(AdminWidgetDescriptor::new(
                AdminWidgetId::new(format!(
                    "ext.{}.{}",
                    handler.extension_id, handler.handler_id
                ))?,
                format!("{} widget", handler.extension_id),
                map_extension_widget_slot(&handler.surface),
                Some(Capability::AdminShellAccess),
                None,
            )?);
        }

        ensure_unique_widgets(&widgets)?;
        Ok(widgets)
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

fn map_bulk_action_kind(kind: CoreBulkOperationKind) -> BulkActionKind {
    match kind {
        CoreBulkOperationKind::Publish => BulkActionKind::Publish,
        CoreBulkOperationKind::Unpublish => BulkActionKind::Custom,
        CoreBulkOperationKind::Reindex => BulkActionKind::Custom,
        CoreBulkOperationKind::Export => BulkActionKind::Export,
        CoreBulkOperationKind::Cancel => BulkActionKind::Cancel,
        CoreBulkOperationKind::CheckIn => BulkActionKind::CheckIn,
        CoreBulkOperationKind::Custom => BulkActionKind::Custom,
    }
}

fn map_extension_widget_slot(surface: &str) -> WidgetSlot {
    if surface.contains("header") {
        WidgetSlot::Header
    } else if surface.contains("sidebar") {
        WidgetSlot::Sidebar
    } else if surface.contains("footer") {
        WidgetSlot::Footer
    } else {
        WidgetSlot::Summary
    }
}
