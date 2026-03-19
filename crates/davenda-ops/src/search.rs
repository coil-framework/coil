use crate::error::OpsModelError;
use crate::identifiers::{SearchFieldId, SearchIndexId};
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_core::{
    SearchFieldContribution as ManifestSearchFieldContribution,
    SearchFieldRole as ManifestSearchFieldRole,
    SearchInvalidationRule as ManifestSearchInvalidationRule,
    SearchInvalidationTrigger as ManifestSearchInvalidationTrigger,
};
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

mod catalog;
mod mapping;

pub use catalog::SearchCatalog;

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

impl fmt::Display for SearchDocumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page => f.write_str("page"),
            Self::Product => f.write_str("product"),
            Self::Collection => f.write_str("collection"),
            Self::Event => f.write_str("event"),
            Self::EventSlot => f.write_str("event_slot"),
            Self::Booking => f.write_str("booking"),
            Self::Media => f.write_str("media"),
            Self::MembershipSubscription => f.write_str("membership_subscription"),
            Self::Custom => f.write_str("custom"),
        }
    }
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
    pub id: SearchFieldId,
    pub source_path: String,
    pub role: SearchFieldRole,
    pub stored: bool,
    pub searchable: bool,
}

impl SearchFieldContribution {
    pub fn new(
        id: SearchFieldId,
        source_path: impl Into<String>,
        role: SearchFieldRole,
        stored: bool,
        searchable: bool,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            id,
            source_path: require_non_empty("search_field_source_path", source_path.into())?,
            role,
            stored,
            searchable,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchInvalidationRule {
    pub trigger: SearchInvalidationTrigger,
    pub reason: String,
}

impl SearchInvalidationRule {
    pub fn new(
        trigger: SearchInvalidationTrigger,
        reason: impl Into<String>,
    ) -> Result<Self, OpsModelError> {
        Ok(Self {
            trigger,
            reason: require_non_empty("search_invalidation_reason", reason.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchIndexContribution {
    pub id: SearchIndexId,
    pub source_module: String,
    pub document_kind: SearchDocumentKind,
    pub visibility: SearchVisibility,
    pub publication_required: bool,
    pub fields: Vec<SearchFieldContribution>,
    pub invalidation_rules: Vec<SearchInvalidationRule>,
    pub rebuild_strategy: SearchRebuildStrategy,
}

impl SearchIndexContribution {
    pub fn new(
        id: SearchIndexId,
        source_module: impl Into<String>,
        document_kind: SearchDocumentKind,
        visibility: SearchVisibility,
        publication_required: bool,
        fields: Vec<SearchFieldContribution>,
        invalidation_rules: Vec<SearchInvalidationRule>,
        rebuild_strategy: SearchRebuildStrategy,
    ) -> Result<Self, OpsModelError> {
        let source_module = require_non_empty("search_source_module", source_module.into())?;

        if fields.is_empty() {
            return Err(OpsModelError::InvalidSearchVisibility {
                index_id: id.to_string(),
                reason: "at least one indexed field is required".to_string(),
            });
        }

        if matches!(visibility, SearchVisibility::Public) && !publication_required {
            return Err(OpsModelError::InvalidSearchVisibility {
                index_id: id.to_string(),
                reason: "public search indexes must require publication state".to_string(),
            });
        }

        let mut seen_fields = HashSet::new();
        for field in &fields {
            if !seen_fields.insert(field.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateField {
                    index_id: id.to_string(),
                    field_id: field.id.to_string(),
                });
            }
        }

        Ok(Self {
            id,
            source_module,
            document_kind,
            visibility,
            publication_required,
            fields,
            invalidation_rules,
            rebuild_strategy,
        })
    }

    pub fn visible_to(&self, capabilities: &[Capability]) -> bool {
        self.visibility.allows(capabilities)
    }
}
