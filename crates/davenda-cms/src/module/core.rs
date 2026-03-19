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
        plan.insert(MigrationStep::new(
            MigrationId::new("001_pages_revisions")?,
            owner.clone(),
            10,
            "create cms pages, localized revisions, and seo metadata tables",
        )?)?;
        plan.insert(MigrationStep::new(
            MigrationId::new("002_navigation")?,
            owner.clone(),
            20,
            "create navigation trees and navigation item adjacency tables",
        )?)?;
        plan.insert(MigrationStep::new(
            MigrationId::new("003_redirects")?,
            owner.clone(),
            30,
            "create redirect rules and route handoff tables",
        )?)?;
        plan.insert(MigrationStep::new(
            MigrationId::new("004_publication_queue")?,
            owner,
            40,
            "create scheduled publication queue and preview token tables",
        )?)?;
        Ok(plan)
    }
}

impl Default for CmsModule {
    fn default() -> Self {
        Self::new()
    }
}
