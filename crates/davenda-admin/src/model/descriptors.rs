use super::*;

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

pub(super) fn map_bulk_action_kind(kind: CoreBulkOperationKind) -> BulkActionKind {
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

pub(super) fn map_extension_widget_slot(surface: &str) -> WidgetSlot {
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
