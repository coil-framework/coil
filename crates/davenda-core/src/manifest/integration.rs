#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationKind {
    AdminNavigation,
    AdminWorkflow,
    FrontendRendering,
    SearchIndex,
    SeoMetadata,
    JsonLd,
    LocalizedContent,
    CacheInvalidation,
    StoragePolicy,
    CommerceBridge,
    AuthPublication,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrationPoint {
    pub kind: IntegrationKind,
    pub surface: String,
    pub description: String,
}

impl IntegrationPoint {
    pub fn new(
        kind: IntegrationKind,
        surface: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            surface: surface.into(),
            description: description.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleBehavior {
    CacheInvalidation,
    LocalizedContent,
    SeoMetadata,
    JsonLd,
    AccessibleAdminUi,
    StoragePolicyAware,
    AuthGovernedPublication,
    AsyncJobs,
    AuditedBulkActions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExtensionSlotKind {
    Page,
    Api,
    Job,
    ScheduledJob,
    Webhook,
    AdminWidget,
    RenderHook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSlotDescriptor {
    pub kind: ExtensionSlotKind,
    pub surface: String,
    pub description: String,
}

impl ExtensionSlotDescriptor {
    pub fn new(
        kind: ExtensionSlotKind,
        surface: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            surface: surface.into(),
            description: description.into(),
        }
    }
}
