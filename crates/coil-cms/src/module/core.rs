use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsModule {
    pub(super) name: String,
    pub(super) config_namespace: String,
    pub(super) admin_resources: Vec<AdminResourceContribution>,
}

impl CmsModule {
    pub fn new() -> Self {
        Self {
            name: "cms".to_string(),
            config_namespace: "cms".to_string(),
            admin_resources: vec![
                AdminResourceContribution::new(
                    "cms.pages",
                    "/admin/cms/pages",
                    "Pages",
                    "Pages",
                    AdminNavigationSection::Content,
                    AdminContributionKind::ResourceIndex,
                    Capability::CmsPageRead,
                ),
                AdminResourceContribution::new(
                    "cms.navigation",
                    "/admin/cms/navigation",
                    "Navigation",
                    "Navigation",
                    AdminNavigationSection::Content,
                    AdminContributionKind::ResourceIndex,
                    Capability::CmsNavigationEdit,
                ),
                AdminResourceContribution::new(
                    "cms.media",
                    "/admin/cms/media",
                    "Media",
                    "Media",
                    AdminNavigationSection::Content,
                    AdminContributionKind::ResourceIndex,
                    Capability::AssetRead,
                ),
            ],
        }
    }

    pub fn admin_resources(&self) -> &[AdminResourceContribution] {
        &self.admin_resources
    }

    pub fn live_pages_query(&self, locale: Option<&str>) -> Result<CmsPageQuery, CmsModelError> {
        let query = QuerySpec::new(
            PageRequest::new(0, 50)?,
            QueryContext {
                locale: locale.map(str::to_owned),
                principal_id: None,
                publication_visibility: PublicationVisibility::PublishedOnly,
                cache_scope: if locale.is_some() {
                    QueryCacheScope::LocaleScoped
                } else {
                    QueryCacheScope::Public
                },
            },
        )
        .with_filter(QueryFilter::new(
            "workflow_status",
            FilterOperator::Eq,
            vec![PageWorkflowStatus::Published.to_string()],
        )?)
        .with_sort(QuerySort::ascending("live_path")?);

        Ok(CmsPageQuery { query })
    }

    pub fn editorial_queue_query(
        &self,
        principal_id: &str,
        locale: Option<&str>,
    ) -> Result<CmsPageQuery, CmsModelError> {
        let query = QuerySpec::new(
            PageRequest::new(0, 100)?,
            QueryContext {
                locale: locale.map(str::to_owned),
                principal_id: Some(require_non_empty("principal_id", principal_id.to_string())?),
                publication_visibility: PublicationVisibility::IncludeDrafts,
                cache_scope: QueryCacheScope::UserScoped,
            },
        )
        .with_filter(QueryFilter::new(
            "workflow_status",
            FilterOperator::In,
            vec![
                PageWorkflowStatus::DraftOnly.to_string(),
                PageWorkflowStatus::Scheduled.to_string(),
                PageWorkflowStatus::PublishedWithDraft.to_string(),
                PageWorkflowStatus::PublishedWithScheduledDraft.to_string(),
            ],
        )?)
        .with_sort(QuerySort::ascending("updated_at")?);

        Ok(CmsPageQuery { query })
    }

    pub fn redirect_lookup_query(
        &self,
        path: &str,
        locale: Option<&str>,
    ) -> Result<RedirectLookupQuery, CmsModelError> {
        let query = QuerySpec::new(
            PageRequest::new(0, 1)?,
            QueryContext {
                locale: locale.map(str::to_owned),
                principal_id: None,
                publication_visibility: PublicationVisibility::PublishedOnly,
                cache_scope: if locale.is_some() {
                    QueryCacheScope::LocaleScoped
                } else {
                    QueryCacheScope::Public
                },
            },
        )
        .with_filter(QueryFilter::new(
            "redirect_from",
            FilterOperator::Eq,
            vec![validate_path("redirect_lookup_path", path.to_string())?],
        )?);

        Ok(RedirectLookupQuery { query })
    }

    pub fn migration_plan(&self) -> Result<MigrationPlan, CmsModelError> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("001_pages_revisions")?,
                owner.clone(),
                10,
                "create cms pages, localized revisions, and seo metadata tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_pages (page_id TEXT PRIMARY KEY, locale TEXT NOT NULL, title TEXT NOT NULL, slug TEXT NOT NULL, template TEXT NOT NULL, body_html TEXT NOT NULL, live_path TEXT NOT NULL, workflow_status TEXT NOT NULL, seo_title TEXT, seo_description TEXT, canonical_path TEXT, media_references TEXT NOT NULL DEFAULT '[]', source_system TEXT, source_key TEXT UNIQUE, import_batch_id TEXT, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("002_navigation")?,
                owner.clone(),
                20,
                "create navigation trees and navigation item adjacency tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_navigation (navigation_id TEXT PRIMARY KEY, locale TEXT, payload TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("003_redirects")?,
                owner.clone(),
                30,
                "create redirect rules and route handoff tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_redirects (redirect_from TEXT PRIMARY KEY, redirect_to TEXT NOT NULL, locale TEXT, permanent BOOLEAN NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("004_publication_queue")?,
                owner,
                40,
                "create scheduled publication queue and preview token tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_publication_queue (page_id TEXT PRIMARY KEY, publish_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_preview_tokens (token TEXT PRIMARY KEY, page_id TEXT NOT NULL, expires_at BIGINT NOT NULL)",
            )?,
        )?;
        Ok(plan)
    }
}

impl Default for CmsModule {
    fn default() -> Self {
        Self::new()
    }
}
