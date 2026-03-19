use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminNavigationSection {
    Overview,
    Content,
    Commerce,
    Memberships,
    Events,
    Media,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminContributionKind {
    Dashboard,
    ResourceIndex,
    DetailView,
    Workflow,
    Audit,
    Settings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminResourceContribution {
    pub id: String,
    pub route: String,
    pub title: String,
    pub nav_label: String,
    pub section: AdminNavigationSection,
    pub kind: AdminContributionKind,
    pub required_capability: Capability,
}

impl AdminResourceContribution {
    pub fn new(
        id: impl Into<String>,
        route: impl Into<String>,
        title: impl Into<String>,
        nav_label: impl Into<String>,
        section: AdminNavigationSection,
        kind: AdminContributionKind,
        required_capability: Capability,
    ) -> Self {
        Self {
            id: id.into(),
            route: route.into(),
            title: title.into(),
            nav_label: nav_label.into(),
            section,
            kind,
            required_capability,
        }
    }
}
