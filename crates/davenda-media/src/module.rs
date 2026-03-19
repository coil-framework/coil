use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaModule {
    name: String,
    config_namespace: String,
}

impl MediaModule {
    pub fn new() -> Self {
        Self {
            name: "media".to_string(),
            config_namespace: "media".to_string(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config_namespace(&self) -> &str {
        &self.config_namespace
    }
}

impl Default for MediaModule {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformModule for MediaModule {
    fn manifest(&self) -> ModuleManifest {
        ModuleManifest::new(self.name.clone())
            .with_required_capabilities(vec![
                Capability::AssetRead,
                Capability::AssetPublish,
                Capability::AssetReplace,
                Capability::AssetManageStorage,
            ])
            .with_optional_capabilities(vec![
                Capability::AssetReadPublic,
                Capability::AdminShellAccess,
                Capability::AdminAuditRead,
                Capability::CmsPageRead,
                Capability::SeoMetadataEdit,
                Capability::I18nTranslationEdit,
            ])
            .with_config_namespace(self.config_namespace.clone())
            .with_capability_contracts(vec![
                CapabilityContract::required(Capability::AssetRead, ["asset", "media"]),
                CapabilityContract::required(
                    Capability::AssetPublish,
                    ["asset", "media"],
                ),
                CapabilityContract::required(
                    Capability::AssetReplace,
                    ["asset", "media"],
                ),
                CapabilityContract::required(
                    Capability::AssetManageStorage,
                    ["asset", "asset_folder", "media_library"],
                ),
                CapabilityContract::optional(
                    Capability::AssetReadPublic,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::AdminShellAccess,
                    ["admin_module"],
                ),
                CapabilityContract::optional(
                    Capability::AdminAuditRead,
                    ["audit_entry"],
                ),
                CapabilityContract::optional(Capability::CmsPageRead, ["page"]),
                CapabilityContract::optional(
                    Capability::SeoMetadataEdit,
                    ["asset", "media"],
                ),
                CapabilityContract::optional(
                    Capability::I18nTranslationEdit,
                    ["asset", "media"],
                ),
            ])
            .with_module_dependencies(vec![
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
            ])
            .with_core_service_dependencies(vec![
                CoreServiceDependency::Auth,
                CoreServiceDependency::Data,
                CoreServiceDependency::Storage,
                CoreServiceDependency::Assets,
                CoreServiceDependency::Jobs,
                CoreServiceDependency::Seo,
                CoreServiceDependency::I18n,
                CoreServiceDependency::Observability,
            ])
            .with_migrations(vec![
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
            ])
            .with_route_surfaces(vec![
                RouteSurface::new(
                    "media.library",
                    RouteSurfaceKind::AdminPage,
                    "/admin/media",
                )
                .gated_by(Capability::AssetRead),
                RouteSurface::new(
                    "media.delivery",
                    RouteSurfaceKind::Asset,
                    "/media/files/{asset_id}",
                )
                .gated_by(Capability::AssetRead),
                RouteSurface::new(
                    "media.storage",
                    RouteSurfaceKind::AdminPage,
                    "/admin/media/storage",
                )
                .gated_by(Capability::AssetManageStorage),
            ])
            .with_jobs(vec![
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
            ])
            .with_event_subscriptions(vec![
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
            ])
            .with_integration_points(vec![
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
            ])
            .with_behaviors(vec![
                ModuleBehavior::StoragePolicyAware,
                ModuleBehavior::AuthGovernedPublication,
                ModuleBehavior::AsyncJobs,
                ModuleBehavior::AccessibleAdminUi,
            ])
            .with_extension_slots(vec![
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
            ])
            .with_admin_resources(vec![
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
            ])
            .with_search_contributions(vec![SearchIndexContribution::new(
                "search.media",
                SearchDocumentKind::Media,
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
            )])
            .with_http_surfaces(vec![
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
            ])
    }

    fn register(&self, registry: &mut ServiceRegistry) -> Result<(), RegistrationError> {
        registry.register_module_service(
            self.name.clone(),
            "module.media.libraries",
            "Media libraries, folder trees, and library-level policy defaults",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.folders",
            "Managed media folders and storage policy overrides",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.assets",
            "Managed media assets, revisions, publication state, and reuse across modules",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.metadata",
            "Metadata capture, derived metadata, and image handling",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.replacement",
            "Replacement workflows and revision promotion for managed assets",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.storage",
            "Storage policy interplay, delivery modes, and local-only overrides",
        )?;
        registry.register_module_service(
            self.name.clone(),
            "module.media.admin",
            "Media admin resources and operator workflows",
        )
    }

    fn install_migration_plan(&self) -> Option<MigrationPlan> {
        let owner = MigrationOwner::Module(self.name.clone());
        let mut plan = MigrationPlan::new();
        plan.insert(
            MigrationStep::new(
                MigrationId::new("media_libraries").expect("constant migration id is valid"),
                owner.clone(),
                10,
                "Create media-library and folder policy storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS media_libraries (id TEXT PRIMARY KEY, name TEXT NOT NULL, default_policy TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("media migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("media_assets").expect("constant migration id is valid"),
                owner.clone(),
                20,
                "Create managed media asset and revision storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS media_assets (id TEXT PRIMARY KEY, library_id TEXT NOT NULL, slug TEXT NOT NULL, status TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("media migration ids are unique");
        plan.insert(
            MigrationStep::new(
                MigrationId::new("media_derivatives").expect("constant migration id is valid"),
                owner,
                30,
                "Create media derivative and sync backlog storage",
            )
            .expect("constant migration step is valid")
            .with_statement(
                "CREATE TABLE IF NOT EXISTS media_derivatives (id TEXT PRIMARY KEY, asset_id TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL)",
            )
            .expect("constant migration statement is valid"),
        )
        .expect("media migration ids are unique");
        Some(plan)
    }
}
