use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDocumentKind {
    Page,
    Product,
    Collection,
    Event,
    EventSlot,
    Booking,
    Media,
    MembershipSubscription,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFieldRole {
    Title,
    Summary,
    Body,
    Keyword,
    Facet,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchVisibility {
    Public,
    Authenticated,
    Capability(Capability),
}

impl SearchVisibility {
    pub fn allows(&self, capabilities: &[Capability]) -> bool {
        match self {
            Self::Public => true,
            Self::Authenticated => !capabilities.is_empty(),
            Self::Capability(capability) => capabilities.contains(capability),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchInvalidationTrigger {
    Published,
    Updated,
    Unpublished,
    Deleted,
    ManualRebuild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchRebuildStrategy {
    OnInvalidate,
    Scheduled { interval: Duration },
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchFieldContribution {
    pub id: String,
    pub source_path: String,
    pub role: SearchFieldRole,
    pub stored: bool,
    pub searchable: bool,
}

impl SearchFieldContribution {
    pub fn new(
        id: impl Into<String>,
        source_path: impl Into<String>,
        role: SearchFieldRole,
        stored: bool,
        searchable: bool,
    ) -> Self {
        Self {
            id: id.into(),
            source_path: source_path.into(),
            role,
            stored,
            searchable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInvalidationRule {
    pub trigger: SearchInvalidationTrigger,
    pub reason: String,
}

impl SearchInvalidationRule {
    pub fn new(trigger: SearchInvalidationTrigger, reason: impl Into<String>) -> Self {
        Self {
            trigger,
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexContribution {
    pub id: String,
    pub document_kind: SearchDocumentKind,
    pub visibility: SearchVisibility,
    pub publication_required: bool,
    pub fields: Vec<SearchFieldContribution>,
    pub invalidation_rules: Vec<SearchInvalidationRule>,
    pub rebuild_strategy: SearchRebuildStrategy,
}

impl SearchIndexContribution {
    pub fn new(
        id: impl Into<String>,
        document_kind: SearchDocumentKind,
        visibility: SearchVisibility,
        publication_required: bool,
        fields: Vec<SearchFieldContribution>,
        invalidation_rules: Vec<SearchInvalidationRule>,
        rebuild_strategy: SearchRebuildStrategy,
    ) -> Self {
        Self {
            id: id.into(),
            document_kind,
            visibility,
            publication_required,
            fields,
            invalidation_rules,
            rebuild_strategy,
        }
    }
}
