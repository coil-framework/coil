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

    pub fn page_builder_inventory_query(
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
            "content_kind",
            FilterOperator::In,
            vec!["structured".to_string(), "hybrid".to_string()],
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
                "create cms pages, localized revisions, page settings, and seo metadata tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_pages (page_id TEXT PRIMARY KEY, locale TEXT NOT NULL, title TEXT NOT NULL, slug TEXT NOT NULL, template TEXT NOT NULL, summary TEXT NOT NULL DEFAULT '', body_html TEXT NOT NULL, content_kind TEXT NOT NULL DEFAULT 'legacy_html', block_count INTEGER NOT NULL DEFAULT 0, has_shared_blocks BOOLEAN NOT NULL DEFAULT FALSE, page_settings TEXT NOT NULL DEFAULT '{}', show_in_navigation BOOLEAN NOT NULL DEFAULT TRUE, allow_indexing BOOLEAN NOT NULL DEFAULT TRUE, include_in_sitemap BOOLEAN NOT NULL DEFAULT TRUE, navigation_label TEXT, layout_variant TEXT, live_path TEXT NOT NULL, workflow_status TEXT NOT NULL, seo_title TEXT, seo_description TEXT, canonical_path TEXT, media_references TEXT NOT NULL DEFAULT '[]', source_system TEXT, source_key TEXT UNIQUE, import_batch_id TEXT, fingerprint TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?,
        )?;
        plan.insert(
            MigrationStep::new(
                MigrationId::new("001b_page_builder")?,
                owner.clone(),
                15,
                "create structured block schema, page block, and shared block tables",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_block_types (block_type_id TEXT PRIMARY KEY, label TEXT NOT NULL, definition TEXT NOT NULL, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_block_type_fields (block_type_id TEXT NOT NULL, field_id TEXT NOT NULL, label TEXT NOT NULL, value_kind TEXT NOT NULL, required BOOLEAN NOT NULL DEFAULT FALSE, multiple BOOLEAN NOT NULL DEFAULT FALSE, position INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (block_type_id, field_id))",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_page_blocks (page_id TEXT NOT NULL, revision_id TEXT NOT NULL, instance_id TEXT NOT NULL, block_type_id TEXT NOT NULL, source_kind TEXT NOT NULL, shared_block_id TEXT, payload TEXT NOT NULL, position INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (page_id, revision_id, instance_id))",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_shared_blocks (shared_block_id TEXT PRIMARY KEY, locale TEXT, label TEXT NOT NULL, block_type_id TEXT NOT NULL, block_payload TEXT NOT NULL, reference_count INTEGER NOT NULL DEFAULT 0, updated_at BIGINT NOT NULL)",
            )?
            .with_statement(
                "CREATE TABLE IF NOT EXISTS cms_shared_block_references (shared_block_id TEXT NOT NULL, page_id TEXT NOT NULL, revision_id TEXT NOT NULL, instance_id TEXT NOT NULL, PRIMARY KEY (shared_block_id, page_id, revision_id, instance_id))",
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
