use super::*;
use davenda_auth::Capability;
use davenda_core::{
    AdminContributionKind as CoreAdminContributionKind,
    AdminNavigationSection as CoreAdminNavigationSection, AdminResourceContribution,
    BulkOperationDefinition as CoreBulkOperationDefinition,
    BulkOperationKind as CoreBulkOperationKind, CoreServiceDependency, ExtensionSlotKind,
    ModuleManifest, PlatformModule, ServiceRegistry,
};
use davenda_wasm::{
    AdminWidgetExtensionPoint, ContractVersion, ExtensionInstallation, ExtensionManifest,
    ExtensionPoint, ExtensionPointKind, ExtensionRegistry, HandlerId, HandlerInstallation,
    HandlerManifest, HostCapabilityGrant, HostGrantSet, InstalledExtension, ResourceLimits,
};

fn installed_admin_extension() -> InstalledExtension {
    InstalledExtension::install(
        ExtensionManifest::new(
            davenda_wasm::ExtensionId::new("admin.waitlist").unwrap(),
            "Waitlist Dashboard Widgets",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::AdminWidget),
            vec![HandlerManifest::new(
                HandlerId::new("waitlist-summary").unwrap(),
                "exports.waitlist_summary",
                ExtensionPoint::AdminWidget(
                    AdminWidgetExtensionPoint::new("admin.dashboard.summary").unwrap(),
                ),
                HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
            )
            .unwrap()],
        )
        .unwrap(),
        ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-summary").unwrap(),
                HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
            )],
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn admin_shell_filters_resources_and_widgets_by_capability() {
    let shell = AdminModule::new().shell().clone();
    let operator = OperatorAccessContext::new().with_capability(Capability::AdminShellAccess);

    let resources = shell.visible_resources(&operator);
    let widgets = shell.visible_widgets(&operator);

    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].route, "/admin");
    assert_eq!(widgets.len(), 1);
    assert_eq!(widgets[0].id, AdminWidgetId::new("admin.status").unwrap());
}

#[test]
fn admin_shell_groups_navigation_and_plans_bulk_actions() {
    let base = AdminModule::new().shell().clone();
    let workflows =
        AdminShell::compose_module_workflows(
            &[ModuleManifest::new("ops").with_bulk_operations(vec![
                CoreBulkOperationDefinition::new(
                    "bulk.reports.export",
                    "Bulk export reports",
                    Some("Queue exports for multiple reports".to_string()),
                    Capability::AdminAuditRead,
                    CoreBulkOperationKind::Export,
                    davenda_core::BulkOperationScope::System,
                    davenda_jobs::RetryPolicy::new(
                        3,
                        std::time::Duration::from_secs(15),
                        std::time::Duration::from_secs(300),
                    )
                    .unwrap(),
                    Some(50),
                    true,
                ),
            ])]
            .as_slice(),
        )
        .unwrap();
    let shell = AdminShell::new(
        base.accessibility().clone(),
        base.resources().to_vec(),
        base.widgets().to_vec(),
        workflows,
    )
    .unwrap();
    let operator = OperatorAccessContext::new()
        .with_capability(Capability::AdminAuditRead)
        .with_capability(Capability::SystemModuleManage);

    let grouped = shell.navigation_by_section(&operator);
    assert_eq!(grouped[&NavigationSection::System].len(), 2);

    let plan = shell
        .build_bulk_action_plan(
            &WorkflowId::new("bulk.reports.export").unwrap(),
            42,
            &operator,
        )
        .unwrap();
    assert_eq!(plan.resource_count, 42);
    assert_eq!(plan.message, "Bulk export reports queued");
}

#[test]
fn audit_visibility_requires_audit_capability() {
    let mut shell = AdminModule::new().shell().clone();
    shell.record_audit_entry(
        AuditEntry::new(
            AuditEntryId::new("audit-1").unwrap(),
            "user-1",
            Capability::CmsPagePublish,
            ResourceKind::new("page").unwrap(),
            "page-123",
            "published",
        )
        .unwrap(),
    );

    let denied = OperatorAccessContext::new().with_capability(Capability::AdminShellAccess);
    assert!(shell.visible_audit_entries(&denied).is_empty());

    let allowed = OperatorAccessContext::new().with_capability(Capability::AdminAuditRead);
    assert_eq!(shell.visible_audit_entries(&allowed).len(), 1);
}

#[test]
fn admin_module_manifest_and_accessibility_contract_are_stable() {
    let module = AdminModule::new();
    let manifest = module.manifest();

    assert_eq!(manifest.name, "admin");
    assert_eq!(
        manifest.required_capabilities,
        vec![Capability::AdminShellAccess, Capability::AdminAuditRead]
    );
    assert!(manifest
        .optional_capabilities
        .contains(&Capability::SystemModuleManage));
    assert_eq!(
        manifest.core_service_dependencies,
        vec![
            CoreServiceDependency::Auth,
            CoreServiceDependency::A11y,
            CoreServiceDependency::Template,
            CoreServiceDependency::I18n,
            CoreServiceDependency::Observability,
            CoreServiceDependency::Http,
        ]
    );
    assert_eq!(manifest.route_surfaces.len(), 2);
    assert_eq!(manifest.http_surfaces.len(), 2);
    assert_eq!(manifest.jobs.len(), 1);
    assert_eq!(manifest.event_subscriptions.len(), 1);
    assert!(manifest
        .extension_slots
        .iter()
        .any(|slot| slot.kind == ExtensionSlotKind::AdminWidget));
    assert_eq!(
        module
            .install_migration_plan()
            .expect("admin migration plan")
            .ordered_steps()
            .len(),
        2
    );

    let accessibility = module.shell().accessibility();
    assert_eq!(accessibility.skip_link_target, "admin-main");
    assert_eq!(accessibility.live_region_id, "admin-status");
    assert!(accessibility.table_caption_required);
}

#[test]
fn module_registration_exposes_admin_services() {
    let module = AdminModule::new();
    let mut registry = ServiceRegistry::new();
    module.register(&mut registry).unwrap();

    assert!(registry
        .services()
        .any(|service| service.id == "module.admin.shell"));
    assert!(registry
        .services()
        .any(|service| service.id == "module.admin.accessibility"));
}

#[test]
fn admin_shell_composes_shared_module_resource_contributions() {
    let manifests = vec![
        ModuleManifest::new("cms").with_admin_resources(vec![AdminResourceContribution::new(
            "cms.pages",
            "/admin/cms/pages",
            "Pages",
            "Pages",
            CoreAdminNavigationSection::Content,
            CoreAdminContributionKind::ResourceIndex,
            Capability::CmsPageRead,
        )]),
        ModuleManifest::new("events").with_admin_resources(vec![AdminResourceContribution::new(
            "events.check-in",
            "/admin/events/check-in",
            "Check-in",
            "Check-in",
            CoreAdminNavigationSection::Events,
            CoreAdminContributionKind::Workflow,
            Capability::EventsBookingCheckIn,
        )]),
    ];

    let resources = AdminShell::compose_module_resources(&manifests).unwrap();
    assert_eq!(resources.len(), 2);
    assert_eq!(resources[0].route, "/admin/cms/pages");
    assert_eq!(resources[0].section, NavigationSection::Content);
    assert_eq!(resources[1].kind, AdminResourceKind::Workflow);
}

#[test]
fn admin_shell_composes_shared_bulk_workflows() {
    let workflows = AdminShell::compose_module_workflows(
        &[ModuleManifest::new("events").with_bulk_operations(vec![
            CoreBulkOperationDefinition::new(
                "bulk.events.check-in",
                "Bulk check in bookings",
                Some("Checks in bookings".to_string()),
                Capability::EventsBookingCheckIn,
                CoreBulkOperationKind::CheckIn,
                davenda_core::BulkOperationScope::Events,
                davenda_jobs::RetryPolicy::new(
                    3,
                    std::time::Duration::from_secs(15),
                    std::time::Duration::from_secs(300),
                )
                .unwrap(),
                Some(200),
                true,
            ),
        ])]
        .as_slice(),
    )
    .unwrap();

    assert_eq!(workflows.len(), 1);
    assert_eq!(
        workflows[0].id,
        WorkflowId::new("bulk.events.check-in").unwrap()
    );
    assert_eq!(workflows[0].bulk_action, BulkActionKind::CheckIn);
}

#[test]
fn admin_shell_composes_extension_widgets_from_registry() {
    let mut registry = ExtensionRegistry::new(ContractVersion::new(1, 0, 0));
    registry.install(installed_admin_extension()).unwrap();

    let widgets = AdminShell::compose_extension_widgets(&registry).unwrap();
    assert_eq!(widgets.len(), 1);
    assert_eq!(
        widgets[0].id,
        AdminWidgetId::new("ext.admin.waitlist.waitlist-summary").unwrap()
    );
    assert_eq!(widgets[0].slot, WidgetSlot::Summary);
    assert_eq!(
        widgets[0].required_capability,
        Some(Capability::AdminShellAccess)
    );
}
