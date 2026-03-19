use super::*;

pub(super) fn build_manifest(module: &MediaModule) -> ModuleManifest {
    ModuleManifest::new(module.name().to_string())
        .with_required_capabilities(required_capabilities())
        .with_optional_capabilities(optional_capabilities())
        .with_config_namespace(module.config_namespace().to_string())
        .with_capability_contracts(capability_contracts())
        .with_module_dependencies(module_dependencies())
        .with_core_service_dependencies(core_service_dependencies())
        .with_migrations(module_migrations())
        .with_route_surfaces(route_surfaces())
        .with_jobs(jobs())
        .with_event_subscriptions(event_subscriptions())
        .with_integration_points(integration_points())
        .with_behaviors(module_behaviors())
        .with_extension_slots(extension_slots())
        .with_admin_resources(admin_resources())
        .with_search_contributions(search_contributions())
        .with_http_surfaces(http_surfaces())
}

fn required_capabilities() -> Vec<Capability> {
    vec![
        Capability::AssetRead,
        Capability::AssetPublish,
        Capability::AssetReplace,
        Capability::AssetManageStorage,
    ]
}

fn optional_capabilities() -> Vec<Capability> {
    vec![
        Capability::AssetReadPublic,
        Capability::AdminShellAccess,
        Capability::AdminAuditRead,
        Capability::CmsPageRead,
        Capability::SeoMetadataEdit,
        Capability::I18nTranslationEdit,
    ]
}

fn capability_contracts() -> Vec<CapabilityContract> {
    vec![
        CapabilityContract::required(Capability::AssetRead, ["asset", "media"]),
        CapabilityContract::required(Capability::AssetPublish, ["asset", "media"]),
        CapabilityContract::required(Capability::AssetReplace, ["asset", "media"]),
        CapabilityContract::required(
            Capability::AssetManageStorage,
            ["asset", "asset_folder", "media_library"],
        ),
        CapabilityContract::optional(Capability::AssetReadPublic, ["asset", "media"]),
        CapabilityContract::optional(Capability::AdminShellAccess, ["admin_module"]),
        CapabilityContract::optional(Capability::AdminAuditRead, ["audit_entry"]),
        CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
        CapabilityContract::optional(Capability::SeoMetadataEdit, ["asset", "media"]),
        CapabilityContract::optional(Capability::I18nTranslationEdit, ["asset", "media"]),
    ]
}

fn module_dependencies() -> Vec<ModuleDependency> {
    vec![
        ModuleDependency::optional(
            "admin",
            "Media contributes library and storage-policy workflows to the shared admin shell when installed",
        ),
        ModuleDependency::optional(
            "cms",
            "Managed assets can be referenced from CMS pages and editorial workflows",
        ),
        ModuleDependency::optional(
            "events",
            "Managed assets can supply event media and downloadable resources",
        ),
        ModuleDependency::optional(
            "commerce",
            "Managed assets can supply product media and downloadable order assets",
        ),
    ]
}

fn core_service_dependencies() -> Vec<CoreServiceDependency> {
    vec![
        CoreServiceDependency::Auth,
        CoreServiceDependency::Data,
        CoreServiceDependency::Storage,
        CoreServiceDependency::Assets,
        CoreServiceDependency::Jobs,
        CoreServiceDependency::Seo,
        CoreServiceDependency::I18n,
        CoreServiceDependency::Observability,
    ]
}

fn module_migrations() -> Vec<MigrationContract> {
    vec![
        MigrationContract::new(
            "media.libraries",
            10,
            "Creates media-library, folder, and inherited storage-policy tables",
        ),
        MigrationContract::new(
            "media.assets",
            20,
            "Creates managed asset, revision, and publication workflow tables",
        ),
        MigrationContract::new(
            "media.derivatives",
            30,
            "Creates derivative generation, sync backlog, and technical metadata tables",
        ),
    ]
}

fn route_surfaces() -> Vec<RouteSurface> {
    vec![
        RouteSurface::new("media.library", RouteSurfaceKind::AdminPage, "/admin/media")
            .gated_by(Capability::AssetRead),
        RouteSurface::new("media.delivery", RouteSurfaceKind::Asset, "/media/files/{asset_id}")
            .gated_by(Capability::AssetRead),
        RouteSurface::new(
            "media.storage",
            RouteSurfaceKind::AdminPage,
            "/admin/media/storage",
        )
        .gated_by(Capability::AssetManageStorage),
    ]
}

fn jobs() -> Vec<JobContract> {
    vec![
        JobContract::new(
            "media.derivatives.generate",
            JobTriggerKind::InlineFollowup,
            true,
            "Generates thumbnails, previews, and responsive derivatives after asset ingest",
        ),
        JobContract::new(
            "media.storage.sync",
            JobTriggerKind::Scheduled,
            true,
            "Reconciles write-through object storage and tracks exceptional local-only assets",
        ),
    ]
}

fn event_subscriptions() -> Vec<EventSubscription> {
    vec![
        EventSubscription::new(
            "media.asset.published",
            Some("media.storage.sync"),
            "Ensures newly published assets are durably available across object storage and delivery surfaces",
        ),
        EventSubscription::new(
            "media.asset.replaced",
            Some("media.derivatives.generate"),
            "Regenerates derived media artifacts when a staged replacement goes live",
        ),
    ]
}

fn integration_points() -> Vec<IntegrationPoint> {
    vec![
        IntegrationPoint::new(
            IntegrationKind::StoragePolicy,
            "media.storage-policy",
            "Bridges folder defaults, per-upload overrides, and delivery modes onto the shared storage engine",
        ),
        IntegrationPoint::new(
            IntegrationKind::AuthPublication,
            "media.publication",
            "Treats managed-asset publication as an auth-governed state transition instead of a file-copy side effect",
        ),
        IntegrationPoint::new(
            IntegrationKind::SeoMetadata,
            "media.metadata",
            "Supplies alt text, dimensions, and canonical metadata for downstream consumers",
        ),
    ]
}

fn module_behaviors() -> Vec<ModuleBehavior> {
    vec![
        ModuleBehavior::StoragePolicyAware,
        ModuleBehavior::AuthGovernedPublication,
        ModuleBehavior::AsyncJobs,
        ModuleBehavior::AccessibleAdminUi,
    ]
}

fn extension_slots() -> Vec<ExtensionSlotDescriptor> {
    vec![
        ExtensionSlotDescriptor::new(
            ExtensionSlotKind::AdminWidget,
            "media.asset.sidebar",
            "Allows bounded customer widgets to augment media detail views and review workflows",
        ),
        ExtensionSlotDescriptor::new(
            ExtensionSlotKind::RenderHook,
            "media.asset.metadata",
            "Allows controlled metadata enrichment without bypassing the shared storage and publication rules",
        ),
    ]
}

fn admin_resources() -> Vec<AdminResourceContribution> {
    vec![
        AdminResourceContribution::new(
            "media.library",
            "/admin/media",
            "Media library",
            "Media",
            AdminNavigationSection::Media,
            AdminContributionKind::ResourceIndex,
            Capability::AssetRead,
        ),
        AdminResourceContribution::new(
            "media.storage",
            "/admin/media/storage",
            "Storage policies",
            "Storage",
            AdminNavigationSection::Media,
            AdminContributionKind::Settings,
            Capability::AssetManageStorage,
        ),
    ]
}

fn search_contributions() -> Vec<SearchIndexContribution> {
    vec![SearchIndexContribution::new(
        "search.media",
        SearchDocumentKind::Media,
        SearchVisibility::Public,
        true,
        vec![
            SearchFieldContribution::new("title", "title", SearchFieldRole::Title, true, true),
            SearchFieldContribution::new(
                "alt",
                "alt_text",
                SearchFieldRole::Metadata,
                true,
                true,
            ),
        ],
        vec![
            SearchInvalidationRule::new(
                SearchInvalidationTrigger::Published,
                "asset published",
            ),
            SearchInvalidationRule::new(
                SearchInvalidationTrigger::Updated,
                "asset replaced",
            ),
        ],
        SearchRebuildStrategy::OnInvalidate,
    )]
}

fn http_surfaces() -> Vec<HttpSurfaceContribution> {
    vec![
        HttpSurfaceContribution::page(
            "media.library",
            HttpSurfaceArea::Admin,
            "/admin/media",
            "media/library",
        )
        .gated_by(Capability::AssetRead),
        HttpSurfaceContribution::file(
            "media.delivery",
            HttpSurfaceArea::Public,
            "/media/files/{asset_id}",
            "media/files/{asset_id}",
            "application/octet-stream",
            HttpFileDeliveryMode::AppProxy,
        )
        .gated_by(Capability::AssetRead),
        HttpSurfaceContribution::page(
            "media.storage",
            HttpSurfaceArea::Admin,
            "/admin/media/storage",
            "media/storage",
        )
        .gated_by(Capability::AssetManageStorage),
    ]
}
