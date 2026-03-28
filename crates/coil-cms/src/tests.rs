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
    assert_eq!(manifest.migrations.len(), 3);
    assert_eq!(manifest.route_surfaces.len(), 10);
    assert_eq!(manifest.http_surfaces.len(), 10);
    assert_eq!(manifest.jobs.len(), 2);
    assert_eq!(manifest.event_subscriptions.len(), 2);
    assert_eq!(manifest.search_contributions.len(), 1);
    assert_eq!(manifest.bulk_operations.len(), 2);
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
            .any(|service| service.id == "module.cms.media_refs")
    );
    assert_eq!(module.admin_resources().len(), 3);
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

    let redirect = module
        .redirect_lookup_query("/legacy/home", Some("en-GB"))
        .unwrap();
    assert_eq!(
        redirect.query.filters[0].values,
        vec!["/legacy/home".to_string()]
    );

    let migrations = module.migration_plan().unwrap();
    assert_eq!(migrations.ordered_steps().len(), 4);
    assert_eq!(
        migrations.ordered_steps()[0].owner,
        MigrationOwner::Module("cms".to_string())
    );
    assert!(
        migrations.ordered_steps()[0]
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE TABLE IF NOT EXISTS cms_pages"))
    );
    assert!(
        migrations.ordered_steps()[3]
            .statements
            .iter()
            .any(|statement| statement.contains("CREATE TABLE IF NOT EXISTS cms_preview_tokens"))
    );
}
