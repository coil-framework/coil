use super::*;
use std::collections::BTreeMap;

use coil_auth::Capability;
use coil_core::{CoreServiceDependency, ExtensionSlotKind, PlatformModule, ServiceRegistry};
use coil_data::{MigrationOwner, PublicationVisibility, QueryCacheScope, TransactionIsolation};

fn revision(id: &str, slug: &str) -> PageRevision {
    PageRevision::new(
        RevisionId::new(id).unwrap(),
        format!("Title {id}"),
        Slug::new(slug).unwrap(),
        TemplateHandle::new("cms.page").unwrap(),
        "<p>Hello</p>",
        SeoMetadata::new(
            Some(format!("SEO {id}")),
            Some("description".to_string()),
            Some(format!("/en-GB/{slug}")),
        )
        .unwrap(),
    )
    .unwrap()
    .with_media_reference(AssetReference::new("asset:hero").unwrap())
}

fn hero_schema() -> BlockTypeSchema {
    BlockTypeSchema::new(
        BlockTypeId::new("hero").unwrap(),
        "Hero",
        vec![
            BlockFieldSchema::new(
                BlockFieldId::new("heading").unwrap(),
                "Heading",
                BlockFieldValueKind::PlainText,
                true,
                false,
            )
            .unwrap(),
            BlockFieldSchema::new(
                BlockFieldId::new("body").unwrap(),
                "Body",
                BlockFieldValueKind::RichText,
                false,
                false,
            )
            .unwrap(),
            BlockFieldSchema::new(
                BlockFieldId::new("artwork").unwrap(),
                "Artwork",
                BlockFieldValueKind::AssetReference,
                false,
                false,
            )
            .unwrap(),
            BlockFieldSchema::new(
                BlockFieldId::new("cta_paths").unwrap(),
                "Call to action paths",
                BlockFieldValueKind::Path,
                false,
                true,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

#[test]
fn cms_module_manifest_declares_expected_capabilities_and_registers_services() {
    let module = CmsModule::new();
    let manifest = module.manifest();
    let mut registry = ServiceRegistry::new();

    module.register(&mut registry).unwrap();

    assert_eq!(manifest.name, "cms");
    assert_eq!(manifest.config_namespace.as_deref(), Some("cms"));
    assert_eq!(
        manifest.required_capabilities,
        vec![
            Capability::CmsPageRead,
            Capability::CmsPageEdit,
            Capability::CmsPagePublish,
            Capability::CmsNavigationEdit,
        ]
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::AdminShellAccess)
    );
    assert!(
        manifest
            .optional_capabilities
            .contains(&Capability::AssetRead)
    );
    assert_eq!(manifest.migrations.len(), 4);
    assert_eq!(manifest.route_surfaces.len(), 18);
    assert_eq!(manifest.http_surfaces.len(), 18);
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.pages.save-settings")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.pages.save-blocks")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.shared-blocks.save")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.options.index")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.options.save")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.pages.schedule")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.pages.rollback")
    );
    assert!(
        manifest
            .route_surfaces
            .iter()
            .any(|surface| surface.name == "cms.pages.duplicate-block")
    );
    assert_eq!(manifest.jobs.len(), 2);
    assert_eq!(manifest.event_subscriptions.len(), 2);
    assert_eq!(manifest.search_contributions.len(), 1);
    assert_eq!(manifest.bulk_operations.len(), 2);
    assert_eq!(manifest.data_repositories.len(), 2);
    assert!(
        manifest
            .module_dependencies
            .iter()
            .any(|dependency| dependency.module == "media")
    );
    assert!(
        manifest
            .core_service_dependencies
            .contains(&CoreServiceDependency::Seo)
    );
    assert!(
        manifest
            .extension_slots
            .iter()
            .any(|slot| slot.kind == ExtensionSlotKind::RenderHook)
    );
    assert_eq!(manifest.admin_resources.len(), 3);
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.cms.pages")
    );
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.cms.page_builder")
    );
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.cms.media_refs")
    );
    assert!(
        registry
            .services()
            .any(|service| service.id == "module.cms.shared_blocks")
    );
    assert!(
        manifest
            .search_contributions[0]
            .fields
            .iter()
            .any(|field| field.id == "summary" && field.role == coil_core::SearchFieldRole::Summary)
    );
    assert!(
        manifest.search_contributions[0]
            .fields
            .iter()
            .any(|field| field.id == "blocks" && field.source_path == "structured_blocks")
    );
    assert!(
        manifest
            .data_repositories
            .iter()
            .any(|repository| repository.id == "cms.shared_blocks")
    );
    assert_eq!(module.admin_resources().len(), 3);
}

#[test]
fn block_type_schema_rejects_duplicate_fields_and_empty_labels() {
    assert_eq!(
        BlockFieldSchema::new(
            BlockFieldId::new("heading").unwrap(),
            "",
            BlockFieldValueKind::PlainText,
            true,
            false,
        )
        .unwrap_err(),
        CmsModelError::EmptyField {
            field: "block_field_label",
        }
    );

    assert_eq!(
        BlockTypeSchema::new(
            BlockTypeId::new("hero").unwrap(),
            "Hero",
            vec![
                BlockFieldSchema::new(
                    BlockFieldId::new("heading").unwrap(),
                    "Heading",
                    BlockFieldValueKind::PlainText,
                    true,
                    false,
                )
                .unwrap(),
                BlockFieldSchema::new(
                    BlockFieldId::new("heading").unwrap(),
                    "Heading again",
                    BlockFieldValueKind::PlainText,
                    false,
                    false,
                )
                .unwrap(),
            ],
        )
        .unwrap_err(),
        CmsModelError::DuplicateBlockFieldSchema {
            block_type_id: "hero".to_string(),
            field_id: "heading".to_string(),
        }
    );
}

#[test]
fn block_type_schema_distinguishes_schema_from_content_instances() {
    let schema = hero_schema();
    let mut fields = BTreeMap::new();
    fields.insert(
        BlockFieldId::new("heading").unwrap(),
        vec![BlockFieldValue::plain_text("Welcome aboard").unwrap()],
    );
    fields.insert(
        BlockFieldId::new("body").unwrap(),
        vec![BlockFieldValue::rich_text("<p>Structured body</p>").unwrap()],
    );
    fields.insert(
        BlockFieldId::new("artwork").unwrap(),
        vec![BlockFieldValue::asset_reference(
            AssetReference::new("asset:hero").unwrap(),
        )],
    );
    fields.insert(
        BlockFieldId::new("cta_paths").unwrap(),
        vec![
            BlockFieldValue::path("/shop").unwrap(),
            BlockFieldValue::path("/memberships").unwrap(),
        ],
    );

    let instance = schema
        .instantiate(BlockInstanceId::new("hero-1").unwrap(), fields)
        .unwrap();

    assert_eq!(instance.block_type.as_str(), "hero");
    assert_eq!(schema.fields().len(), 4);
    assert_eq!(instance.fields.len(), 4);
}

#[test]
fn block_type_schema_rejects_invalid_content_instances() {
    let schema = hero_schema();

    let mut missing_required = BTreeMap::new();
    missing_required.insert(
        BlockFieldId::new("body").unwrap(),
        vec![BlockFieldValue::rich_text("<p>Body only</p>").unwrap()],
    );
    assert_eq!(
        schema
            .instantiate(
                BlockInstanceId::new("hero-missing").unwrap(),
                missing_required,
            )
            .unwrap_err(),
        CmsModelError::MissingRequiredBlockField {
            block_type_id: "hero".to_string(),
            field_id: "heading".to_string(),
        }
    );

    let mut wrong_kind = BTreeMap::new();
    wrong_kind.insert(
        BlockFieldId::new("heading").unwrap(),
        vec![BlockFieldValue::boolean(true)],
    );
    assert_eq!(
        schema
            .instantiate(BlockInstanceId::new("hero-wrong-kind").unwrap(), wrong_kind)
            .unwrap_err(),
        CmsModelError::InvalidBlockFieldValueKind {
            block_type_id: "hero".to_string(),
            field_id: "heading".to_string(),
            expected: BlockFieldValueKind::PlainText,
            actual: BlockFieldValueKind::Boolean,
        }
    );

    let mut unknown_field = BTreeMap::new();
    unknown_field.insert(
        BlockFieldId::new("heading").unwrap(),
        vec![BlockFieldValue::plain_text("Hello").unwrap()],
    );
    unknown_field.insert(
        BlockFieldId::new("unknown").unwrap(),
        vec![BlockFieldValue::plain_text("Mystery").unwrap()],
    );
    assert_eq!(
        schema
            .instantiate(BlockInstanceId::new("hero-unknown").unwrap(), unknown_field)
            .unwrap_err(),
        CmsModelError::UnknownBlockField {
            block_type_id: "hero".to_string(),
            field_id: "unknown".to_string(),
        }
    );

    let mut too_many_values = BTreeMap::new();
    too_many_values.insert(
        BlockFieldId::new("heading").unwrap(),
        vec![
            BlockFieldValue::plain_text("One").unwrap(),
            BlockFieldValue::plain_text("Two").unwrap(),
        ],
    );
    assert_eq!(
        schema
            .instantiate(
                BlockInstanceId::new("hero-multiple-headings").unwrap(),
                too_many_values,
            )
            .unwrap_err(),
        CmsModelError::BlockFieldDoesNotAllowMultiple {
            block_type_id: "hero".to_string(),
            field_id: "heading".to_string(),
        }
    );
}

#[test]
fn page_revision_supports_settings_inline_blocks_and_shared_block_references() {
    let schema = hero_schema();
    let inline_block = schema
        .instantiate(
            BlockInstanceId::new("hero-inline").unwrap(),
            BTreeMap::from([(
                BlockFieldId::new("heading").unwrap(),
                vec![BlockFieldValue::plain_text("Inline hero").unwrap()],
            )]),
        )
        .unwrap();
    let shared_block = SharedBlock::new(
        SharedBlockId::new("shared-footer-callout").unwrap(),
        "Footer callout",
        schema
            .instantiate(
                BlockInstanceId::new("shared-hero").unwrap(),
                BTreeMap::from([(
                    BlockFieldId::new("heading").unwrap(),
                    vec![BlockFieldValue::plain_text("Shared hero").unwrap()],
                )]),
            )
            .unwrap(),
    )
    .unwrap();

    let settings = PageSettings::new(PageOptions {
        show_in_navigation: false,
        allow_indexing: true,
        include_in_sitemap: false,
    })
    .with_navigation_label("Campaign landing")
    .unwrap()
    .with_layout_variant("marketing.hero")
    .unwrap();

    let revision = revision("rev-hero", "launch")
        .with_settings(settings.clone())
        .with_inline_block(inline_block.clone())
        .unwrap()
        .with_shared_block_reference(
            shared_block.reference(BlockInstanceId::new("footer").unwrap()),
        )
        .unwrap();

    assert_eq!(revision.settings, settings);
    assert_eq!(revision.blocks.len(), 2);
    assert_eq!(revision.blocks[0].instance_id().as_str(), "hero-inline");
    assert_eq!(revision.blocks[0].block_type().as_str(), "hero");
    assert_eq!(revision.blocks[1].instance_id().as_str(), "footer");
    assert_eq!(revision.blocks[1].block_type().as_str(), "hero");
}

#[test]
fn page_revision_rejects_duplicate_block_instances_and_shared_blocks_preserve_content_identity() {
    let schema = hero_schema();
    let inline_block = schema
        .instantiate(
            BlockInstanceId::new("hero-inline").unwrap(),
            BTreeMap::from([(
                BlockFieldId::new("heading").unwrap(),
                vec![BlockFieldValue::plain_text("Inline hero").unwrap()],
            )]),
        )
        .unwrap();
    let shared_block = SharedBlock::new(
        SharedBlockId::new("shared-hero").unwrap(),
        "Reusable hero",
        inline_block.clone(),
    )
    .unwrap();

    assert_eq!(shared_block.block.id.as_str(), "hero-inline");
    assert_eq!(
        revision("rev-duplicate", "duplicate")
            .with_inline_block(inline_block)
            .unwrap()
            .with_shared_block_reference(
                shared_block.reference(BlockInstanceId::new("hero-inline").unwrap())
            )
            .unwrap_err(),
        CmsModelError::DuplicatePageBlockInstance {
            instance_id: "hero-inline".to_string(),
        }
    );
}

#[test]
fn page_settings_default_to_searchable_navigation_friendly_options() {
    let settings = PageSettings::default();

    assert_eq!(settings.navigation_label, None);
    assert_eq!(settings.layout_variant, None);
    assert!(settings.options.show_in_navigation);
    assert!(settings.options.allow_indexing);
    assert!(settings.options.include_in_sitemap);
}

#[test]
fn page_workflow_keeps_live_revision_until_new_revision_is_published() {
    let mut page = CmsPage::new(
        PageId::new("page-home").unwrap(),
        LocaleCode::new("en-GB").unwrap(),
        revision("rev-1", "home"),
    );

    assert_eq!(page.workflow_status(), PageWorkflowStatus::DraftOnly);
    page.publish_current();
    assert_eq!(page.workflow_status(), PageWorkflowStatus::Published);
    assert_eq!(page.live_revision().unwrap().id.as_str(), "rev-1");

    page.replace_draft(revision("rev-2", "home-launch"));
    assert_eq!(
        page.workflow_status(),
        PageWorkflowStatus::PublishedWithDraft
    );
    assert_eq!(page.preview_revision().id.as_str(), "rev-2");
    assert_eq!(page.live_revision().unwrap().id.as_str(), "rev-1");

    page.publish_current();
    assert_eq!(page.workflow_status(), PageWorkflowStatus::Published);
    assert_eq!(page.live_revision().unwrap().id.as_str(), "rev-2");
}

#[test]
fn page_scheduling_requires_a_future_timestamp_and_promotes_when_due() {
    let mut page = CmsPage::new(
        PageId::new("page-event").unwrap(),
        LocaleCode::new("en-GB").unwrap(),
        revision("rev-1", "event"),
    );

    assert_eq!(
        page.schedule_current(100, 100).unwrap_err(),
        CmsModelError::CannotScheduleInThePast {
            publish_at: 100,
            now: 100,
        }
    );

    page.schedule_current(150, 100).unwrap();
    assert_eq!(page.workflow_status(), PageWorkflowStatus::Scheduled);
    assert!(!page.apply_schedule(149));
    assert_eq!(page.workflow_status(), PageWorkflowStatus::Scheduled);
    assert!(page.apply_schedule(150));
    assert_eq!(page.workflow_status(), PageWorkflowStatus::Published);
    assert_eq!(page.live_path().unwrap(), "/en-GB/event");
}

#[test]
fn unpublishing_preserves_draft_but_removes_live_route() {
    let mut page = CmsPage::new(
        PageId::new("page-about").unwrap(),
        LocaleCode::new("en-GB").unwrap(),
        revision("rev-1", "about"),
    );
    page.publish_current();

    page.unpublish().unwrap();

    assert_eq!(page.workflow_status(), PageWorkflowStatus::Unpublished);
    assert_eq!(
        page.live_path().unwrap_err(),
        CmsModelError::MissingLiveRevision {
            page_id: "page-about".to_string(),
        }
    );
    assert_eq!(page.preview_path(), "/en-GB/about");
}

#[test]
fn navigation_resolution_filters_out_unpublished_pages() {
    let mut live_page = CmsPage::new(
        PageId::new("page-home").unwrap(),
        LocaleCode::new("en-GB").unwrap(),
        revision("rev-home", "home"),
    );
    live_page.publish_current();

    let draft_page = CmsPage::new(
        PageId::new("page-secret").unwrap(),
        LocaleCode::new("en-GB").unwrap(),
        revision("rev-secret", "secret"),
    );

    let pages = BTreeMap::from([
        (live_page.id.clone(), live_page),
        (draft_page.id.clone(), draft_page),
    ]);

    let tree = NavigationTree::new(
        NavigationId::new("main-nav").unwrap(),
        vec![
            NavigationItem::page(
                NavigationItemId::new("home").unwrap(),
                "Home",
                PageId::new("page-home").unwrap(),
            )
            .unwrap(),
            NavigationItem::page(
                NavigationItemId::new("secret").unwrap(),
                "Secret",
                PageId::new("page-secret").unwrap(),
            )
            .unwrap(),
            NavigationItem::external(
                NavigationItemId::new("docs").unwrap(),
                "Docs",
                "/support/docs",
            )
            .unwrap(),
        ],
    )
    .unwrap();

    let resolved = tree.resolve(&pages).unwrap();

    assert_eq!(resolved.len(), 2);
    assert_eq!(resolved[0].href, "/en-GB/home");
    assert_eq!(resolved[1].href, "/support/docs");
}

#[test]
fn navigation_tree_rejects_duplicate_item_ids() {
    let item = NavigationItem::external(
        NavigationItemId::new("dup").unwrap(),
        "Docs",
        "/support/docs",
    )
    .unwrap();

    assert_eq!(
        NavigationTree::new(
            NavigationId::new("main-nav").unwrap(),
            vec![item.clone(), item],
        )
        .unwrap_err(),
        CmsModelError::DuplicateNavigationItem {
            item_id: "dup".to_string(),
        }
    );
}

#[test]
fn cms_module_exposes_queries_migrations_and_transaction_plans() {
    let mut page = CmsPage::new(
        PageId::new("page-home").unwrap(),
        LocaleCode::new("en-GB").unwrap(),
        revision("rev-home", "home"),
    );
    page.publish_current();
    page.schedule_current(200, 100).unwrap();

    let save_draft = page.save_draft_transaction_plan().unwrap();
    assert_eq!(save_draft.writes.len(), 2);
    assert_eq!(
        save_draft.after_commit_events,
        vec!["cms.page.draft_saved:page-home".to_string()]
    );

    let publish = page.publish_transaction_plan().unwrap();
    assert_eq!(publish.isolation, TransactionIsolation::Serializable);
    assert!(
        publish
            .writes
            .iter()
            .any(|write| write.resource == "sitemap_entry")
    );

    let schedule = page.schedule_transaction_plan().unwrap();
    assert!(
        schedule
            .after_commit_jobs
            .iter()
            .any(|job| job == "cms.jobs.publication_schedule.enqueue:page-home")
    );

    let unpublish = page.unpublish_transaction_plan().unwrap();
    assert!(
        unpublish
            .writes
            .iter()
            .any(|write| write.action == "delete")
    );

    let module = CmsModule::new();
    let live_query = module.live_pages_query(Some("en-GB")).unwrap();
    assert_eq!(
        live_query.query.context.cache_scope,
        QueryCacheScope::LocaleScoped
    );
    assert_eq!(
        live_query.query.context.publication_visibility,
        PublicationVisibility::PublishedOnly
    );
    assert_eq!(
        live_query.query.filters[0].values,
        vec!["published".to_string()]
    );

    let editorial = module
        .editorial_queue_query("editor-7", Some("en-GB"))
        .unwrap();
    assert_eq!(
        editorial.query.context.principal_id.as_deref(),
        Some("editor-7")
    );
    assert_eq!(
        editorial.query.context.cache_scope,
        QueryCacheScope::UserScoped
    );

    let page_builder = module
        .page_builder_inventory_query("editor-7", Some("en-GB"))
        .unwrap();
    assert_eq!(
        page_builder.query.context.principal_id.as_deref(),
        Some("editor-7")
    );
    assert_eq!(
        page_builder.query.context.cache_scope,
        QueryCacheScope::UserScoped
    );
    assert_eq!(page_builder.query.filters[0].field.as_str(), "content_kind");
    assert_eq!(
        page_builder.query.filters[0].values,
        vec!["structured".to_string(), "hybrid".to_string()]
    );
    assert_eq!(page_builder.query.sort[0].field.as_str(), "updated_at");

    let redirect = module
        .redirect_lookup_query("/legacy/home", Some("en-GB"))
        .unwrap();
    assert_eq!(
        redirect.query.filters[0].values,
        vec!["/legacy/home".to_string()]
    );

    let migrations = module.migration_plan().unwrap();
    assert_eq!(migrations.ordered_steps().len(), 5);
    assert_eq!(
        migrations.ordered_steps()[0].owner,
        MigrationOwner::Module("cms".to_string())
    );
    assert!(
        migrations.ordered_steps()[0]
            .statements
            .iter()
            .any(|statement| {
                statement.contains("CREATE TABLE IF NOT EXISTS cms_pages")
                    && statement.contains("page_settings TEXT")
                    && statement.contains("content_kind TEXT")
            })
    );
    assert!(
        migrations.ordered_steps()[1]
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE TABLE IF NOT EXISTS cms_shared_blocks"))
    );
    assert!(
        migrations.ordered_steps()[1]
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE TABLE IF NOT EXISTS cms_page_blocks"))
    );
    assert!(
        migrations.ordered_steps()[4]
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE TABLE IF NOT EXISTS cms_preview_tokens"))
    );
}
