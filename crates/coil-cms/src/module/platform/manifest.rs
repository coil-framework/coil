use super::super::core::CmsModule;
use super::super::support::{cms_live_pages_repository, default_retry_policy};
use coil_auth::Capability;
use coil_core::{
    BulkOperationDefinition, BulkOperationKind, BulkOperationScope, CapabilityContract,
    CoreServiceDependency, EventSubscription, ExtensionSlotDescriptor, ExtensionSlotKind,
    HttpSurfaceArea, HttpSurfaceContribution, IntegrationKind, IntegrationPoint, JobContract,
    JobTriggerKind, MigrationContract, ModuleBehavior, ModuleDependency, ModuleManifest,
    RouteSurface, RouteSurfaceKind, SearchDocumentKind, SearchFieldContribution, SearchFieldRole,
    SearchIndexContribution, SearchInvalidationRule, SearchInvalidationTrigger,
    SearchRebuildStrategy, SearchVisibility,
};

pub(super) fn build_manifest(module: &CmsModule) -> ModuleManifest {
    ModuleManifest::new(module.name.clone())
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
            .with_config_namespace(module.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(Capability::CmsPageRead, ["page"]),
                CapabilityContract::required(Capability::CmsPageEdit, ["page"]),
                CapabilityContract::required(Capability::CmsPagePublish, ["page"]),
                CapabilityContract::required(Capability::CmsNavigationEdit, ["navigation"]),
                CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
                CapabilityContract::optional(Capability::SeoMetadataEdit, ["page", "navigation"]),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["page", "navigation"],
                ),
                CapabilityContract::optional(Capability::AssetRead, ["asset", "media"]),
                CapabilityContract::optional(Capability::AssetReadPublic, ["asset", "media"]),
                CapabilityContract::optional(Capability::AssetPublish, ["asset", "media"]),
                CapabilityContract::optional(Capability::AssetReplace, ["asset", "media"]),
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
                RouteSurface::new("cms.pages.index", RouteSurfaceKind::AdminPage, "/admin/pages")
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
                RouteSurface::new(
                    "cms.pages.save-draft",
                    RouteSurfaceKind::AdminAction,
                    "/admin/pages/draft",
                )
                .gated_by(Capability::CmsPageEdit),
                RouteSurface::new(
                    "cms.pages.publish",
                    RouteSurfaceKind::AdminAction,
                    "/admin/pages/publish",
                )
                .gated_by(Capability::CmsPagePublish),
                RouteSurface::new(
                    "cms.pages.unpublish",
                    RouteSurfaceKind::AdminAction,
                    "/admin/pages/unpublish",
                )
                .gated_by(Capability::CmsPagePublish),
                RouteSurface::new(
                    "cms.navigation.save",
                    RouteSurfaceKind::AdminAction,
                    "/admin/navigation/save",
                )
                .gated_by(Capability::CmsNavigationEdit),
                RouteSurface::new(
                    "cms.redirects.save",
                    RouteSurfaceKind::AdminAction,
                    "/admin/redirects/save",
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
            .with_admin_resources(module.admin_resources.clone())
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
                HttpSurfaceContribution::redirect(
                    "cms.pages.save-draft",
                    coil_core::HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/pages/draft",
                    "/admin/pages",
                    303,
                )
                .gated_by(Capability::CmsPageEdit),
                HttpSurfaceContribution::redirect(
                    "cms.pages.publish",
                    coil_core::HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/pages/publish",
                    "/admin/pages",
                    303,
                )
                .gated_by(Capability::CmsPagePublish),
                HttpSurfaceContribution::redirect(
                    "cms.pages.unpublish",
                    coil_core::HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/pages/unpublish",
                    "/admin/pages",
                    303,
                )
                .gated_by(Capability::CmsPagePublish),
                HttpSurfaceContribution::redirect(
                    "cms.navigation.save",
                    coil_core::HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/navigation/save",
                    "/admin/navigation",
                    303,
                )
                .gated_by(Capability::CmsNavigationEdit),
                HttpSurfaceContribution::redirect(
                    "cms.redirects.save",
                    coil_core::HttpSurfaceMethod::Post,
                    HttpSurfaceArea::Admin,
                    "/admin/redirects/save",
                    "/admin/redirects",
                    303,
                )
                .gated_by(Capability::CmsPageEdit),
            ])
}
