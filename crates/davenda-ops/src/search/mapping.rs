use super::*;
use davenda_core::{
    SearchDocumentKind as ManifestSearchDocumentKind,
    SearchIndexContribution as ManifestSearchIndexContribution,
    SearchRebuildStrategy as ManifestSearchRebuildStrategy,
    SearchVisibility as ManifestSearchVisibility,
};

impl SearchFieldContribution {
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

impl SearchInvalidationRule {
    pub(crate) fn from_manifest_rule(
        rule: &ManifestSearchInvalidationRule,
    ) -> Result<Self, OpsModelError> {
        Self::new(
            map_search_invalidation_trigger(rule.trigger),
            rule.reason.clone(),
        )
    }
}

impl SearchIndexContribution {
    pub(crate) fn from_manifest_contribution(
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
