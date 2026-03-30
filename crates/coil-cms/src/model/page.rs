use super::*;

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
    pub settings: PageSettings,
    pub blocks: Vec<PageBlockInstance>,
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
            settings: PageSettings::default(),
            blocks: Vec::new(),
        })
    }

    pub fn with_media_reference(mut self, asset: AssetReference) -> Self {
        self.media_references.insert(asset);
        self
    }

    pub fn with_settings(mut self, settings: PageSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn with_inline_block(
        mut self,
        block: StructuredBlockInstance,
    ) -> Result<Self, CmsModelError> {
        self.push_block_instance(PageBlockInstance::Inline(block))?;
        Ok(self)
    }

    pub fn with_shared_block_reference(
        mut self,
        reference: SharedBlockReference,
    ) -> Result<Self, CmsModelError> {
        self.push_block_instance(PageBlockInstance::Shared(reference))?;
        Ok(self)
    }

    fn push_block_instance(&mut self, block: PageBlockInstance) -> Result<(), CmsModelError> {
        let instance_id = block.instance_id().to_string();
        if self
            .blocks
            .iter()
            .any(|existing| existing.instance_id() == block.instance_id())
        {
            return Err(CmsModelError::DuplicatePageBlockInstance { instance_id });
        }

        self.blocks.push(block);
        Ok(())
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
pub struct RedirectRule {
    pub from_path: String,
    pub to_path: String,
    pub permanent: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsPageQuery {
    pub query: QuerySpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectLookupQuery {
    pub query: QuerySpec,
}
