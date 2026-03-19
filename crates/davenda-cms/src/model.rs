use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::time::Duration;

use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind, AdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition, BulkOperationKind, BulkOperationScope, CapabilityContract,
    CoreServiceDependency, DataRepositoryContribution, DataRepositoryQueryProfile,
    EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind, HttpSurfaceArea,
    HttpSurfaceContribution, IntegrationKind, IntegrationPoint, JobContract, JobTriggerKind,
    MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest, PlatformModule,
    RegistrationError, RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution,
    SearchFieldRole, SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility, ServiceRegistry,
};
use davenda_data::{
    DataModelError, DomainWrite, FilterOperator, MigrationId, MigrationOwner, MigrationPlan,
    MigrationStep, PageRequest, PublicationVisibility, QueryCacheScope, QueryContext, QueryField,
    QueryFilter, QuerySort, QuerySpec, RepositorySpec, TableName, TransactionIsolation,
    TransactionPlan,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CmsModelError {
    EmptyField { field: &'static str },
    InvalidToken { field: &'static str, value: String },
    InvalidPath { field: &'static str, value: String },
    DataPlan { error: DataModelError },
    MissingLiveRevision { page_id: String },
    CannotScheduleInThePast { publish_at: u64, now: u64 },
    NavigationCycle { item_id: String },
    DuplicateNavigationItem { item_id: String },
}

impl fmt::Display for CmsModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidPath { field, value } => {
                write!(f, "`{field}` must start with `/`, got `{value}`")
            }
            Self::DataPlan { error } => write!(f, "{error}"),
            Self::MissingLiveRevision { page_id } => {
                write!(f, "page `{page_id}` has no live revision")
            }
            Self::CannotScheduleInThePast { publish_at, now } => write!(
                f,
                "scheduled publish time `{publish_at}` must be greater than current time `{now}`"
            ),
            Self::NavigationCycle { item_id } => {
                write!(f, "navigation item `{item_id}` introduces a cycle")
            }
            Self::DuplicateNavigationItem { item_id } => {
                write!(f, "navigation item `{item_id}` is duplicated in the tree")
            }
        }
    }
}

impl Error for CmsModelError {}

impl From<DataModelError> for CmsModelError {
    fn from(error: DataModelError) -> Self {
        Self::DataPlan { error }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(String);

impl PageId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("page_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionId(String);

impl RevisionId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("revision_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationId(String);

impl NavigationId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("navigation_id", value.into())?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NavigationItemId(String);

impl NavigationItemId {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("navigation_item_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NavigationItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocaleCode(String);

impl LocaleCode {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("locale", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LocaleCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

impl Slug {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("slug", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateHandle(String);

impl TemplateHandle {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("template_handle", value.into())?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetReference(String);

impl AssetReference {
    pub fn new(value: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self(validate_token("asset_reference", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_path: Option<String>,
}

impl SeoMetadata {
    pub fn new(
        title: Option<String>,
        description: Option<String>,
        canonical_path: Option<String>,
    ) -> Result<Self, CmsModelError> {
        if let Some(path) = canonical_path.as_ref() {
            validate_path("canonical_path", path.clone())?;
        }

        Ok(Self {
            title,
            description,
            canonical_path,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRevision {
    pub id: RevisionId,
    pub title: String,
    pub slug: Slug,
    pub template: TemplateHandle,
    pub body_html: String,
    pub seo: SeoMetadata,
    pub media_references: BTreeSet<AssetReference>,
}

impl PageRevision {
    pub fn new(
        id: RevisionId,
        title: impl Into<String>,
        slug: Slug,
        template: TemplateHandle,
        body_html: impl Into<String>,
        seo: SeoMetadata,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            title: require_non_empty("title", title.into())?,
            slug,
            template,
            body_html: require_non_empty("body_html", body_html.into())?,
            seo,
            media_references: BTreeSet::new(),
        })
    }

    pub fn with_media_reference(mut self, asset: AssetReference) -> Self {
        self.media_references.insert(asset);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageWorkflowStatus {
    DraftOnly,
    Scheduled,
    Published,
    PublishedWithDraft,
    PublishedWithScheduledDraft,
    Unpublished,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationState {
    live_revision: Option<PageRevision>,
    scheduled_publish_at: Option<u64>,
    was_ever_published: bool,
}

impl PublicationState {
    pub fn live_revision(&self) -> Option<&PageRevision> {
        self.live_revision.as_ref()
    }

    pub fn scheduled_publish_at(&self) -> Option<u64> {
        self.scheduled_publish_at
    }

    pub fn is_live(&self) -> bool {
        self.live_revision.is_some()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsPage {
    pub id: PageId,
    pub locale: LocaleCode,
    pub current_revision: PageRevision,
    publication: PublicationState,
}

impl CmsPage {
    pub fn new(id: PageId, locale: LocaleCode, initial_revision: PageRevision) -> Self {
        Self {
            id,
            locale,
            current_revision: initial_revision,
            publication: PublicationState {
                live_revision: None,
                scheduled_publish_at: None,
                was_ever_published: false,
            },
        }
    }

    pub fn publication(&self) -> &PublicationState {
        &self.publication
    }

    pub fn workflow_status(&self) -> PageWorkflowStatus {
        match (
            self.publication.live_revision.as_ref(),
            self.publication.scheduled_publish_at,
            self.publication.was_ever_published,
            self.publication
                .live_revision
                .as_ref()
                .map(|revision| revision.id != self.current_revision.id)
                .unwrap_or(false),
        ) {
            (Some(_), Some(_), _, _) => PageWorkflowStatus::PublishedWithScheduledDraft,
            (None, Some(_), _, _) => PageWorkflowStatus::Scheduled,
            (Some(_), None, _, true) => PageWorkflowStatus::PublishedWithDraft,
            (Some(_), None, _, false) => PageWorkflowStatus::Published,
            (None, None, true, _) => PageWorkflowStatus::Unpublished,
            (None, None, false, _) => PageWorkflowStatus::DraftOnly,
        }
    }

    pub fn preview_revision(&self) -> &PageRevision {
        &self.current_revision
    }

    pub fn live_revision(&self) -> Result<&PageRevision, CmsModelError> {
        self.publication
            .live_revision()
            .ok_or_else(|| CmsModelError::MissingLiveRevision {
                page_id: self.id.to_string(),
            })
    }

    pub fn replace_draft(&mut self, revision: PageRevision) {
        self.current_revision = revision;
    }

    pub fn publish_current(&mut self) {
        self.publication.live_revision = Some(self.current_revision.clone());
        self.publication.scheduled_publish_at = None;
        self.publication.was_ever_published = true;
    }

    pub fn schedule_current(&mut self, publish_at: u64, now: u64) -> Result<(), CmsModelError> {
        if publish_at <= now {
            return Err(CmsModelError::CannotScheduleInThePast { publish_at, now });
        }

        self.publication.scheduled_publish_at = Some(publish_at);
        Ok(())
    }

    pub fn apply_schedule(&mut self, now: u64) -> bool {
        if self
            .publication
            .scheduled_publish_at
            .is_some_and(|publish_at| publish_at <= now)
        {
            self.publish_current();
            true
        } else {
            false
        }
    }

    pub fn unpublish(&mut self) -> Result<(), CmsModelError> {
        self.live_revision()?;
        self.publication.live_revision = None;
        self.publication.scheduled_publish_at = None;
        self.publication.was_ever_published = true;
        Ok(())
    }

    pub fn live_path(&self) -> Result<String, CmsModelError> {
        let live = self.live_revision()?;
        Ok(format!("/{}/{}", self.locale, live.slug.as_str()))
    }

    pub fn preview_path(&self) -> String {
        format!("/{}/{}", self.locale, self.current_revision.slug.as_str())
    }

    pub fn save_draft_transaction_plan(&self) -> Result<TransactionPlan, CmsModelError> {
        TransactionPlan::new("cms.page.save_draft", TransactionIsolation::ReadCommitted)?
            .with_write(DomainWrite::new("cms_page", "upsert")?)
            .with_write(DomainWrite::new("cms_revision", "insert")?)
            .with_after_commit_event(format!("cms.page.draft_saved:{}", self.id))
            .map_err(Into::into)
    }

    pub fn publish_transaction_plan(&self) -> Result<TransactionPlan, CmsModelError> {
        TransactionPlan::new("cms.page.publish", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("cms_page", "update")?)
            .with_write(DomainWrite::new("route_projection", "upsert")?)
            .with_write(DomainWrite::new("sitemap_entry", "upsert")?)
            .with_after_commit_job(format!("cms.jobs.cache_invalidate:{}", self.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!("cms.page.published:{}", self.id))
            })
            .map_err(Into::into)
    }

    pub fn schedule_transaction_plan(&self) -> Result<TransactionPlan, CmsModelError> {
        TransactionPlan::new("cms.page.schedule", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("cms_page", "update")?)
            .with_write(DomainWrite::new("publication_schedule", "upsert")?)
            .with_after_commit_job(format!("cms.jobs.publication_schedule.enqueue:{}", self.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!("cms.page.scheduled:{}", self.id))
            })
            .map_err(Into::into)
    }

    pub fn unpublish_transaction_plan(&self) -> Result<TransactionPlan, CmsModelError> {
        TransactionPlan::new("cms.page.unpublish", TransactionIsolation::Serializable)?
            .with_write(DomainWrite::new("cms_page", "update")?)
            .with_write(DomainWrite::new("route_projection", "delete")?)
            .with_write(DomainWrite::new("sitemap_entry", "delete")?)
            .with_after_commit_job(format!("cms.jobs.cache_invalidate:{}", self.id))
            .and_then(|plan| {
                plan.with_after_commit_event(format!("cms.page.unpublished:{}", self.id))
            })
            .map_err(Into::into)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    Page(PageId),
    ExternalUrl(String),
}

impl NavigationTarget {
    pub fn external(url: impl Into<String>) -> Result<Self, CmsModelError> {
        Ok(Self::ExternalUrl(validate_path(
            "external_url",
            url.into(),
        )?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationItem {
    pub id: NavigationItemId,
    pub label: String,
    pub target: NavigationTarget,
    pub children: Vec<NavigationItem>,
}

impl NavigationItem {
    pub fn page(
        id: NavigationItemId,
        label: impl Into<String>,
        page_id: PageId,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            label: require_non_empty("navigation_label", label.into())?,
            target: NavigationTarget::Page(page_id),
            children: Vec::new(),
        })
    }

    pub fn external(
        id: NavigationItemId,
        label: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            id,
            label: require_non_empty("navigation_label", label.into())?,
            target: NavigationTarget::external(url)?,
            children: Vec::new(),
        })
    }

    pub fn with_child(mut self, child: NavigationItem) -> Self {
        self.children.push(child);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNavigationItem {
    pub id: NavigationItemId,
    pub label: String,
    pub href: String,
    pub children: Vec<ResolvedNavigationItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationTree {
    pub id: NavigationId,
    pub items: Vec<NavigationItem>,
}

impl NavigationTree {
    pub fn new(id: NavigationId, items: Vec<NavigationItem>) -> Result<Self, CmsModelError> {
        let tree = Self { id, items };
        tree.validate()?;
        Ok(tree)
    }

    pub fn validate(&self) -> Result<(), CmsModelError> {
        let mut seen = BTreeSet::new();
        for item in &self.items {
            validate_navigation_item(item, &mut Vec::new(), &mut seen)?;
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        pages: &BTreeMap<PageId, CmsPage>,
    ) -> Result<Vec<ResolvedNavigationItem>, CmsModelError> {
        let mut resolved = Vec::new();
        for item in &self.items {
            if let Some(item) = resolve_navigation_item(item, pages)? {
                resolved.push(item);
            }
        }
        Ok(resolved)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectRule {
    pub from_path: String,
    pub to_path: String,
    pub permanent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsPageQuery {
    pub query: QuerySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectLookupQuery {
    pub query: QuerySpec,
}

impl RedirectRule {
    pub fn new(
        from_path: impl Into<String>,
        to_path: impl Into<String>,
        permanent: bool,
    ) -> Result<Self, CmsModelError> {
        Ok(Self {
            from_path: validate_path("redirect_from", from_path.into())?,
            to_path: validate_path("redirect_to", to_path.into())?,
            permanent,
        })
    }
}

fn validate_navigation_item(
    item: &NavigationItem,
    stack: &mut Vec<NavigationItemId>,
    seen: &mut BTreeSet<NavigationItemId>,
) -> Result<(), CmsModelError> {
    if stack.contains(&item.id) {
        return Err(CmsModelError::NavigationCycle {
            item_id: item.id.to_string(),
        });
    }

    if !seen.insert(item.id.clone()) {
        return Err(CmsModelError::DuplicateNavigationItem {
            item_id: item.id.to_string(),
        });
    }

    stack.push(item.id.clone());
    for child in &item.children {
        validate_navigation_item(child, stack, seen)?;
    }
    stack.pop();

    Ok(())
}

fn resolve_navigation_item(
    item: &NavigationItem,
    pages: &BTreeMap<PageId, CmsPage>,
) -> Result<Option<ResolvedNavigationItem>, CmsModelError> {
    let href = match &item.target {
        NavigationTarget::ExternalUrl(url) => url.clone(),
        NavigationTarget::Page(page_id) => match pages.get(page_id) {
            Some(page) if page.publication().is_live() => page.live_path()?,
            _ => return Ok(None),
        },
    };

    let mut children = Vec::new();
    for child in &item.children {
        if let Some(child) = resolve_navigation_item(child, pages)? {
            children.push(child);
        }
    }

    Ok(Some(ResolvedNavigationItem {
        id: item.id.clone(),
        label: item.label.clone(),
        href,
        children,
    }))
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, CmsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CmsModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, CmsModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(CmsModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn validate_path(field: &'static str, value: String) -> Result<String, CmsModelError> {
    let path = require_non_empty(field, value)?;
    if path.starts_with('/') {
        Ok(path)
    } else {
        Err(CmsModelError::InvalidPath { field, value: path })
    }
}

impl fmt::Display for PageWorkflowStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DraftOnly => f.write_str("draft_only"),
            Self::Scheduled => f.write_str("scheduled"),
            Self::Published => f.write_str("published"),
            Self::PublishedWithDraft => f.write_str("published_with_draft"),
            Self::PublishedWithScheduledDraft => f.write_str("published_with_scheduled_draft"),
            Self::Unpublished => f.write_str("unpublished"),
        }
    }
}
