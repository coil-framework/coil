use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmsModule {
    name: String,
    config_namespace: String,
    admin_resources: Vec<AdminResourceContribution>,
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

impl PlatformModule for CmsModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::CmsPageRead,
                Capability::CmsPageEdit,
                Capability::CmsPagePublish,
                Capability::CmsNavigationEdit,
            ])
            .with_optional_capabilities(vec![
                Capability::AdminShellAccess,
                Capability::SeoMetadataEdit,
                Capability::I18nTranslationEdit,
                Capability::AssetRead,
                Capability::AssetReadPublic,
                Capability::AssetPublish,
                Capability::AssetReplace,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(Capability::CmsPageRead, ["page"]),
                CapabilityContract::required(Capability::CmsPageEdit, ["page"]),
                CapabilityContract::required(Capability::CmsPagePublish, ["page"]),
                CapabilityContract::required(
                    Capability::CmsNavigationEdit,
                    ["navigation"],
                ),
                CapabilityContract::optional(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::optional(
                    Capability::SeoMetadataEdit,
                    ["page", "navigation"],
                ),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["page", "navigation"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
                CapabilityContract::optional(
                    Capability::AssetReadPublic,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AssetPublish,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AssetReplace,
                    ["asset", "media"],
                ),
            ])
            .with_module_dependencies(vec![
                ModuleDependency::optional(
                    "admin",
                    "CMS contributes editor and navigation resources into the shared admin shell when installed",
                ),
                ModuleDependency::optional(
                    "media",
                    "CMS pages can reference managed assets through the shared media library",
                ),
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Cache,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Storage,
                CoreServiceDependency::I18n,
                CoreServiceDependency::Seo,
                CoreServiceDependency::Template,
                CoreServiceDependency::A11y,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
                MigrationContract::new(
                    "cms.pages",
                    10,
                    "Creates localized page, revision, and publication workflow tables",
                ),
                MigrationContract::new(
                    "cms.navigation",
                    20,
                    "Creates navigation trees and editorial route bindings",
                ),
                MigrationContract::new(
                    "cms.redirects",
                    30,
                    "Creates redirect rules and route handoff metadata",
                ),
            ])
            .with_route_surfaces(vec![
                RouteSurface::new("cms.page", RouteSurfaceKind::FrontendPage, "/pages/{slug}")
                    .localized(),
                RouteSurface::new(
                    "cms.preview",
                    RouteSurfaceKind::Fragment,
                    "/admin/pages/preview",
                )
                .gated_by(Capability::CmsPageRead),
                RouteSurface::new(
                    "cms.pages.index",
                    RouteSurfaceKind::AdminPage,
                    "/admin/pages",
                )
                .gated_by(Capability::CmsPageRead),
                RouteSurface::new(
                    "cms.navigation.index",
                    RouteSurfaceKind::AdminPage,
                    "/admin/navigation",
                )
                .gated_by(Capability::CmsNavigationEdit),
                RouteSurface::new(
                    "cms.redirects.index",
                    RouteSurfaceKind::AdminPage,
                    "/admin/redirects",
                )
                .gated_by(Capability::CmsPageEdit),
            ])
            .with_jobs(vec![
                JobContract::new(
                    "cms.publish-scheduled",
                    JobTriggerKind::Scheduled,
                    true,
                    "Promotes scheduled revisions into the live site at their publish window",
                ),
                JobContract::new(
                    "cms.cache.invalidate",
                    JobTriggerKind::DomainEvent,
                    true,
                    "Invalidates navigation, sitemap, and page caches after editorial changes",
                ),
            ])
            .with_event_subscriptions(vec![
                EventSubscription::new(
                    "cms.page.publish-requested",
                    Some("cms.publish-scheduled"),
                    "Schedules future publication work for editorial workflows",
                ),
                EventSubscription::new(
                    "media.asset.published",
                    Some("cms.cache.invalidate"),
                    "Refreshes page fragments that depend on newly published managed assets",
                ),
            ])
            .with_integration_points(vec![
                IntegrationPoint::new(
                    IntegrationKind::AdminNavigation,
                    "admin.content",
                    "Adds page, navigation, and redirect resources to the shared content section",
                ),
                IntegrationPoint::new(
                    IntegrationKind::FrontendRendering,
                    "page.render",
                    "Owns page composition on top of the shared HTML-first template engine",
                ),
                IntegrationPoint::new(
                    IntegrationKind::SeoMetadata,
                    "page.head",
                    "Emits localized canonical metadata, robots directives, and sitemap entries",
                ),
                IntegrationPoint::new(
                    IntegrationKind::JsonLd,
                    "page.schema",
                    "Supplies structured data fragments for page and navigation surfaces",
                ),
                IntegrationPoint::new(
                    IntegrationKind::CacheInvalidation,
                    "page.publish",
                    "Invalidates route, navigation, sitemap, and metadata fragments when publication changes",
                ),
            ])
            .with_behaviors(vec![
                ModuleBehavior::CacheInvalidation,
                ModuleBehavior::LocalizedContent,
                ModuleBehavior::SeoMetadata,
                ModuleBehavior::JsonLd,
                ModuleBehavior::AccessibleAdminUi,
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AuthGovernedPublication,
            ])
            .with_extension_slots(vec![
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::AdminWidget,
                    "cms.page.editor.sidebar",
                    "Allows customer app widgets to augment the page editor without owning the editor runtime",
                ),
                ExtensionSlotDescriptor::new(
                    ExtensionSlotKind::RenderHook,
                    "cms.page.render",
                    "Allows bounded content embellishments during page rendering",
                ),
            ])
            .with_admin_resources(self.admin_resources.clone())
            .with_search_contributions(vec![SearchIndexContribution::new(
                "search.cms.pages",
                SearchDocumentKind::Page,
                SearchVisibility::Public,
                true,
                vec![
                    SearchFieldContribution::new(
                        "title",
                        "title",
                        SearchFieldRole::Title,
                        true,
                        true,
                    ),
                    SearchFieldContribution::new(
                        "body",
                        "body_html",
                        SearchFieldRole::Body,
                        false,
                        true,
                    ),
                    SearchFieldContribution::new(
                        "seo",
                        "seo",
                        SearchFieldRole::Metadata,
                        true,
                        false,
                    ),
                ],
                vec![
                    SearchInvalidationRule::new(
                        SearchInvalidationTrigger::Published,
                        "page published",
                    ),
                    SearchInvalidationRule::new(
                        SearchInvalidationTrigger::Updated,
                        "page updated",
                    ),
                    SearchInvalidationRule::new(
                        SearchInvalidationTrigger::Unpublished,
                        "page unpublished",
                    ),
                ],
                SearchRebuildStrategy::OnInvalidate,
            )])
            .with_bulk_operations(vec![
                BulkOperationDefinition::new(
                    "bulk.cms.publish",
                    "Bulk publish pages",
                    Some("Publishes editorially approved pages through idempotent background work".to_string()),
                    Capability::CmsPagePublish,
                    BulkOperationKind::Publish,
                    BulkOperationScope::Cms,
                    default_retry_policy(),
                    Some(500),
                    true,
                ),
                BulkOperationDefinition::new(
                    "bulk.cms.unpublish",
                    "Bulk unpublish pages",
                    Some("Withdraws published pages without requiring per-row request-time mutations".to_string()),
                    Capability::CmsPagePublish,
                    BulkOperationKind::Unpublish,
                    BulkOperationScope::Cms,
                    default_retry_policy(),
                    Some(500),
                    true,
                ),
            ])
            .with_data_repositories(vec![cms_live_pages_repository()])
            .with_http_surfaces(vec![
                HttpSurfaceContribution::page(
                    "cms.page",
                    HttpSurfaceArea::Public,
                    "/pages/{slug}",
                    "cms/page",
                )
                .localized(),
                HttpSurfaceContribution::fragment(
                    "cms.preview",
                    "/admin/pages/preview",
                    "cms/preview",
                    "preview-pane",
                )
                .gated_by(Capability::CmsPageRead),
                HttpSurfaceContribution::page(
                    "cms.pages.index",
                    HttpSurfaceArea::Admin,
                    "/admin/pages",
                    "cms/pages",
                )
                .gated_by(Capability::CmsPageRead),
                HttpSurfaceContribution::page(
                    "cms.navigation.index",
                    HttpSurfaceArea::Admin,
                    "/admin/navigation",
                    "cms/navigation",
                )
                .gated_by(Capability::CmsNavigationEdit),
                HttpSurfaceContribution::page(
                    "cms.redirects.index",
                    HttpSurfaceArea::Admin,
                    "/admin/redirects",
                    "cms/redirects",
                )
                .gated_by(Capability::CmsPageEdit),
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.cms.pages",
            "CMS page definitions, revisions, and publication workflow",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.cms.navigation",
            "CMS navigation trees and localized route composition",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.cms.redirects",
            "CMS redirects and route handoff rules",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.cms.admin",
            "CMS admin resources, editorial workflow screens, and previews",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.cms.media_refs",
            "CMS media references bound to managed assets and publication state",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        Some(CmsModule::migration_plan(self).expect("cms migration plan is constant and valid"))
    }
}

fn cms_live_pages_repository() -> DataRepositoryContribution {
    DataRepositoryContribution::new(
        RepositorySpec::new(
            "cms.pages.live",
            TableName::new("davenda.cms_pages").expect("constant cms table is valid"),
            vec![
                QueryField::new("page_id").expect("constant cms field is valid"),
                QueryField::new("title").expect("constant cms field is valid"),
                QueryField::new("live_path").expect("constant cms field is valid"),
                QueryField::new("updated_at").expect("constant cms field is valid"),
            ],
        )
        .expect("constant cms repository is valid")
        .with_locale_field("locale")
        .expect("constant cms locale field is valid")
        .with_publication_field("workflow_status", "published")
        .expect("constant cms publication field is valid")
        .with_filterable_field("slug")
        .expect("constant cms filter field is valid")
        .with_sortable_field("live_path")
        .expect("constant cms sortable field is valid")
        .with_default_sort(QuerySort::ascending("live_path").expect("constant cms sort is valid")),
        DataRepositoryQueryProfile::new(
            PageRequest::new(0, 24).expect("constant cms page size is valid"),
            PublicationVisibility::PublishedOnly,
            QueryCacheScope::Public,
        )
        .with_localized_cache_scope(QueryCacheScope::LocaleScoped),
    )
}
