use crate::error::OpsModelError;
use crate::identifiers::{SearchFieldId, SearchIndexId};
use crate::validation::require_non_empty;
use davenda_auth::Capability;
use davenda_core::{
    ModuleManifest, SearchDocumentKind as ManifestSearchDocumentKind,
    SearchFieldContribution as ManifestSearchFieldContribution,
    SearchFieldRole as ManifestSearchFieldRole,
    SearchIndexContribution as ManifestSearchIndexContribution,
    SearchInvalidationRule as ManifestSearchInvalidationRule,
    SearchInvalidationTrigger as ManifestSearchInvalidationTrigger,
    SearchRebuildStrategy as ManifestSearchRebuildStrategy,
    SearchVisibility as ManifestSearchVisibility,
};
use std::collections::HashSet;
use std::fmt;
use std::time::Duration;

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

    pub(crate) fn from_manifest_contribution(
        contribution: &ManifestSearchFieldContribution,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            SearchFieldId::new(contribution.id.clone())?,
            contribution.source_path.clone(),
            map_search_field_role(contribution.role),
            contribution.stored,
            contribution.searchable,
        )
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

    fn from_manifest_rule(rule: &ManifestSearchInvalidationRule) -> Result<Self, OpsModelError> {
        Self::new(
            map_search_invalidation_trigger(rule.trigger),
            rule.reason.clone(),
        )
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

    fn from_manifest_contribution(
        source_module: &str,
        contribution: &ManifestSearchIndexContribution,
    ) -> Result<Self, OpsModelError> {
        let fields = contribution
            .fields
            .iter()
            .map(SearchFieldContribution::from_manifest_contribution)
            .collect::<Result<Vec<_>, _>>()?;
        let invalidation_rules = contribution
            .invalidation_rules
            .iter()
            .map(SearchInvalidationRule::from_manifest_rule)
            .collect::<Result<Vec<_>, _>>()?;

        Self::new(
            SearchIndexId::new(contribution.id.clone())?,
            source_module,
            map_search_document_kind(contribution.document_kind),
            map_search_visibility(contribution.visibility),
            contribution.publication_required,
            fields,
            invalidation_rules,
            map_search_rebuild_strategy(contribution.rebuild_strategy),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchCatalog {
    pub contributions: Vec<SearchIndexContribution>,
}

impl SearchCatalog {
    pub fn new(contributions: Vec<SearchIndexContribution>) -> Self {
        Self { contributions }
    }

    pub fn standard() -> Self {
        Self {
            contributions: Vec::new(),
        }
    }

    pub fn from_manifests(manifests: &[ModuleManifest]) -> Result<Self, OpsModelError> {
        let mut contributions = Vec::new();
        for manifest in manifests {
            for contribution in &manifest.search_contributions {
                contributions.push(SearchIndexContribution::from_manifest_contribution(
                    &manifest.name,
                    contribution,
                )?);
            }
        }
        let catalog = Self::new(contributions);
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), OpsModelError> {
        let mut seen = HashSet::new();
        for contribution in &self.contributions {
            if !seen.insert(contribution.id.as_str().to_string()) {
                return Err(OpsModelError::DuplicateIdentifier {
                    kind: "search index",
                    id: contribution.id.to_string(),
                });
            }
            SearchIndexContribution::new(
                contribution.id.clone(),
                contribution.source_module.clone(),
                contribution.document_kind,
                contribution.visibility,
                contribution.publication_required,
                contribution.fields.clone(),
                contribution.invalidation_rules.clone(),
                contribution.rebuild_strategy,
            )?;
        }

        Ok(())
    }

    pub fn contribution(&self, id: &SearchIndexId) -> Option<&SearchIndexContribution> {
        self.contributions
            .iter()
            .find(|contribution| &contribution.id == id)
    }

    pub fn visible_to(&self, capabilities: &[Capability]) -> Vec<&SearchIndexContribution> {
        self.contributions
            .iter()
            .filter(|contribution| contribution.visible_to(capabilities))
            .collect()
    }
}

impl Default for SearchCatalog {
    fn default() -> Self {
        Self::standard()
    }
}

fn map_search_document_kind(kind: ManifestSearchDocumentKind) -> SearchDocumentKind {
    match kind {
        ManifestSearchDocumentKind::Page => SearchDocumentKind::Page,
        ManifestSearchDocumentKind::Product => SearchDocumentKind::Product,
        ManifestSearchDocumentKind::Collection => SearchDocumentKind::Collection,
        ManifestSearchDocumentKind::Event => SearchDocumentKind::Event,
        ManifestSearchDocumentKind::EventSlot => SearchDocumentKind::EventSlot,
        ManifestSearchDocumentKind::Booking => SearchDocumentKind::Booking,
        ManifestSearchDocumentKind::Media => SearchDocumentKind::Media,
        ManifestSearchDocumentKind::MembershipSubscription => {
            SearchDocumentKind::MembershipSubscription
        }
        ManifestSearchDocumentKind::Custom => SearchDocumentKind::Custom,
    }
}

fn map_search_field_role(role: ManifestSearchFieldRole) -> SearchFieldRole {
    match role {
        ManifestSearchFieldRole::Title => SearchFieldRole::Title,
        ManifestSearchFieldRole::Summary => SearchFieldRole::Summary,
        ManifestSearchFieldRole::Body => SearchFieldRole::Body,
        ManifestSearchFieldRole::Keyword => SearchFieldRole::Keyword,
        ManifestSearchFieldRole::Facet => SearchFieldRole::Facet,
        ManifestSearchFieldRole::Metadata => SearchFieldRole::Metadata,
    }
}

fn map_search_visibility(visibility: ManifestSearchVisibility) -> SearchVisibility {
    match visibility {
        ManifestSearchVisibility::Public => SearchVisibility::Public,
        ManifestSearchVisibility::Authenticated => SearchVisibility::Authenticated,
        ManifestSearchVisibility::Capability(capability) => {
            SearchVisibility::Capability(capability)
        }
    }
}

fn map_search_invalidation_trigger(
    trigger: ManifestSearchInvalidationTrigger,
) -> SearchInvalidationTrigger {
    match trigger {
        ManifestSearchInvalidationTrigger::Published => SearchInvalidationTrigger::Published,
        ManifestSearchInvalidationTrigger::Updated => SearchInvalidationTrigger::Updated,
        ManifestSearchInvalidationTrigger::Unpublished => SearchInvalidationTrigger::Unpublished,
        ManifestSearchInvalidationTrigger::Deleted => SearchInvalidationTrigger::Deleted,
        ManifestSearchInvalidationTrigger::ManualRebuild => {
            SearchInvalidationTrigger::ManualRebuild
        }
    }
}

fn map_search_rebuild_strategy(strategy: ManifestSearchRebuildStrategy) -> SearchRebuildStrategy {
    match strategy {
        ManifestSearchRebuildStrategy::OnInvalidate => SearchRebuildStrategy::OnInvalidate,
        ManifestSearchRebuildStrategy::Scheduled { interval } => {
            SearchRebuildStrategy::Scheduled { interval }
        }
        ManifestSearchRebuildStrategy::ManualOnly => SearchRebuildStrategy::ManualOnly,
    }
}
