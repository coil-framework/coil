use super::*;
use davenda_customer_sdk::{
    AuditFacade, AuthFacade, BackendError, CheckoutHooks, CommerceFacade, CustomerPluginDescriptor,
    OrderDraft, OrderReviewDecision, RequestContext,
};
use davenda_storage::StoragePolicyOverride;
use davenda_template::{DocumentRenderRequest, RenderModel, TemplateName, TemplateSelector};
use std::path::Path;
use std::{fs, sync::Arc};

#[derive(Debug)]
struct ExampleCheckoutPlugin;

#[derive(Debug)]
struct ExampleCheckoutHooks;

impl CheckoutHooks for ExampleCheckoutHooks {
    fn review_order(
        &self,
        _ctx: &RequestContext,
        _order: &OrderDraft,
        _commerce: &dyn CommerceFacade,
        _auth: &dyn AuthFacade,
        _audit: &dyn AuditFacade,
    ) -> Result<OrderReviewDecision, BackendError> {
        Ok(OrderReviewDecision::approved())
    }
}

impl CustomerBackendPlugin for ExampleCheckoutPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new("harbor-shop-backend", "Harbor Shop Backend", "0.1.0")
    }

    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        registry.register_checkout_hooks(Arc::new(ExampleCheckoutHooks))?;
        Ok(())
    }
}

#[derive(Debug)]
struct DuplicateCustomerPlugin;

impl CustomerBackendPlugin for DuplicateCustomerPlugin {
    fn descriptor(&self) -> CustomerPluginDescriptor {
        CustomerPluginDescriptor::new("harbor-shop-backend", "Harbor Shop Backend", "0.1.1")
    }

    fn register(&self, _registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError> {
        Ok(())
    }
}

fn copy_directory(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_directory(&source, &target);
        } else {
            fs::copy(&source, &target).unwrap();
        }
    }
}

fn write_customer_root_manifest(root: &Path, auth_package: &str, enabled_modules: &[&str]) {
    let enabled = enabled_modules
        .iter()
        .map(|module| format!("\"{module}\""))
        .collect::<Vec<_>>()
        .join(", ");
    fs::write(
        root.join("app.toml"),
        format!(
            r#"[app]
name = "showcase-events"
display_name = "Showcase Events"

[domains]
canonical = "www.example.com"
additional = []

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[theme]
active = "showcase"
template_namespaces = ["customer-app"]
asset_roots = []

[auth]
mode = "extend"
package = "{auth_package}"

[modules]
enabled = [{enabled}]
"#
        ),
    )
    .unwrap();
}

#[test]
fn storage_host_applies_path_rules_for_sensitive_files() {
    let config = single_node_valid_config();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_storage_policy_rule(
            PathPolicyRule::new(
                "secure/reports",
                Some(StorageClass::LocalOnlySensitive),
                StoragePolicy::single_node_sensitive(),
            )
            .unwrap()
            .with_local_subdir("sensitive")
            .unwrap(),
        )
        .build()
        .unwrap();

    let storage_plan = plan
        .storage_host()
        .plan_single_node_escape_hatch_write(StoragePlanRequest::new("secure/reports/march.csv"))
        .unwrap();

    assert_eq!(storage_plan.storage_class, StorageClass::LocalOnlySensitive);
    assert_eq!(
        storage_plan.matched_rule_prefix.as_deref(),
        Some("secure/reports")
    );
    assert_eq!(
        storage_plan.local_path.as_deref(),
        Some("/tmp/davenda-runtime-tests/sensitive/secure/reports/march.csv")
    );
    assert_eq!(
        storage_plan.deployment_scope,
        StorageDeploymentScope::SingleNodeOnly
    );
    assert!(storage_plan.requires_single_node());
}

#[test]
fn storage_host_publishes_deployment_releases_to_the_cdn_manifest() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .build()
        .unwrap();
    let digest = "a".repeat(64);
    let artifact = DeploymentArtifact::new(
        "theme/site.css",
        format!("theme/site.{digest}.css"),
        content_fingerprint('a'),
        "text/css",
        512,
    )
    .unwrap();
    let release =
        DeploymentRelease::new(ReleaseId::new("release-20260319").unwrap(), [artifact]).unwrap();

    let manifest = plan
        .storage_host()
        .publish_deployment_release(&release)
        .unwrap();
    let published = manifest.resolve("theme/site.css").unwrap();

    assert_eq!(manifest.release_id().as_str(), "release-20260319");
    assert_eq!(published.delivery().audience(), DeliveryAudience::Public);
    assert!(published.delivery().immutable());
    assert!(matches!(
        published.delivery().target(),
        AssetDeliveryTarget::Cdn { public_url, object_key }
            if public_url == &format!("https://cdn.example.com/theme/site.{digest}.css")
                && object_key == &format!("theme/site.{digest}.css")
    ));
}

#[test]
fn runtime_builder_registers_linked_customer_plugins_through_sdk_hooks() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .register_customer_plugin(ExampleCheckoutPlugin)
        .build()
        .unwrap();

    assert_eq!(plan.customer_hooks.checkout.len(), 1);
    assert_eq!(plan.customer_hooks.cms.len(), 0);
    assert_eq!(plan.customer_hooks.verified_webhooks.len(), 0);
    assert_eq!(plan.linked_customer_plugins.len(), 1);
    assert_eq!(
        plan.linked_customer_plugins[0].plugin_id,
        "harbor-shop-backend"
    );
    assert_eq!(
        plan.linked_customer_plugins[0].display_name,
        "Harbor Shop Backend"
    );
    assert_eq!(plan.linked_customer_plugins[0].version, "0.1.0");
    assert_eq!(
        plan.linked_customer_plugins[0].registered_hooks,
        vec![RegisteredHookKind::Checkout]
    );
}

#[test]
fn runtime_builder_rejects_duplicate_linked_customer_plugins() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let error = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .register_customer_plugin(ExampleCheckoutPlugin)
        .register_customer_plugin(DuplicateCustomerPlugin)
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeBuildError::DuplicateCustomerPlugin { plugin_id }
            if plugin_id == "harbor-shop-backend"
    ));
}

#[test]
fn customer_root_runtime_builder_makes_linked_customer_bootstrap_explicit() {
    let config = PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "enabled = [\"cms-pages\", \"admin-shell\"]",
        "enabled = [\"cms\"]",
    ))
    .unwrap();
    let customer_root = unique_temp_template_root("customer-root-runtime-builder");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <head>
    <title>Customer Root</title>
  </head>
  <body>
    <main>customer-root-runtime</main>
  </body>
</html>"#,
    );
    write_customer_root_manifest(&customer_root, "platform-default-auth", &["cms"]);

    let plan = RuntimeBuilder::for_customer_root(config, DefaultAuthModelPackage::default())
        .with_customer_root(&customer_root)
        .register_module(davenda_cms::CmsModule::new())
        .with_linked_customer_plugin(ExampleCheckoutPlugin)
        .build()
        .unwrap();

    let html = plan
        .template
        .runtime
        .render_document(
            &plan.template.namespace_chain(None),
            DocumentRenderRequest::new(
                TemplateSelector::new(TemplateName::new("pages/home").unwrap()),
                RenderModel::new(),
            ),
        )
        .unwrap()
        .html;

    fs::remove_dir_all(&customer_root).unwrap();

    assert_eq!(plan.linked_customer_plugins.len(), 1);
    assert_eq!(
        plan.linked_customer_plugins[0].plugin_id,
        "harbor-shop-backend"
    );
    assert!(html.contains("customer-root-runtime"), "{html}");
}

#[test]
fn customer_root_runtime_builder_supports_register_aliases() {
    let config = PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "enabled = [\"cms-pages\", \"admin-shell\"]",
        "enabled = [\"cms\"]",
    ))
    .unwrap();
    let customer_root = unique_temp_template_root("customer-root-runtime-register-aliases");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>customer-root-runtime</main></body>
</html>"#,
    );
    write_customer_root_manifest(&customer_root, "platform-default-auth", &["cms"]);

    let plan = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .with_customer_root(&customer_root)
        .register_module(davenda_cms::CmsModule::new())
        .register_customer_plugin(ExampleCheckoutPlugin)
        .build()
        .unwrap();

    fs::remove_dir_all(&customer_root).unwrap();

    assert!(plan.modules.iter().any(|module| module.name == "cms"));
    assert_eq!(plan.linked_customer_plugins.len(), 1);
}

#[test]
fn customer_root_runtime_builder_loads_config_and_auth_from_paths() {
    let customer_root = unique_temp_template_root("customer-root-runtime-path-bootstrap");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>bootstrapped-from-paths</main></body>
</html>"#,
    );
    fs::write(
        customer_root.join("platform.toml"),
        VALID_CONFIG
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"harbor-auth\"",
            )
            .replace(
                "enabled = [\"cms-pages\", \"admin-shell\"]",
                "enabled = [\"cms\"]",
            ),
    )
    .unwrap();
    write_customer_root_manifest(&customer_root, "harbor-auth", &["cms"]);
    let harbor_auth =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/harbor-shop/auth/harbor-auth");
    copy_directory(&harbor_auth, &customer_root.join("auth/harbor-auth"));

    let plan = customer_root_runtime_from_paths(&customer_root, "platform.toml")
        .unwrap()
        .register_module(davenda_cms::CmsModule::new())
        .build()
        .unwrap();

    let html = plan
        .template
        .runtime
        .render_document(
            &plan.template.namespace_chain(None),
            DocumentRenderRequest::new(
                TemplateSelector::new(TemplateName::new("pages/home").unwrap()),
                RenderModel::new(),
            ),
        )
        .unwrap()
        .html;

    fs::remove_dir_all(&customer_root).unwrap();

    assert_eq!(plan.auth_package_name, "harbor-auth");
    assert!(html.contains("bootstrapped-from-paths"), "{html}");
}

#[test]
fn customer_root_bootstrap_inputs_load_config_and_auth_from_paths() {
    let customer_root = unique_temp_template_root("customer-root-bootstrap-inputs");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>bootstrap-inputs</main></body>
</html>"#,
    );
    fs::write(
        customer_root.join("platform.toml"),
        VALID_CONFIG
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"harbor-auth\"",
            )
            .replace(
                "enabled = [\"cms-pages\", \"admin-shell\"]",
                "enabled = [\"cms\"]",
            ),
    )
    .unwrap();
    write_customer_root_manifest(&customer_root, "harbor-auth", &["cms"]);
    let harbor_auth =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/harbor-shop/auth/harbor-auth");
    copy_directory(&harbor_auth, &customer_root.join("auth/harbor-auth"));

    let inputs =
        customer_root_bootstrap_inputs_from_paths(&customer_root, "platform.toml").unwrap();

    fs::remove_dir_all(&customer_root).unwrap();

    assert_eq!(inputs.auth_package_name, "harbor-auth");
    assert_eq!(inputs.config.app.name, "showcase-events");
    assert!(inputs.config_path.ends_with("platform.toml"));
}

#[test]
fn customer_root_runtime_builder_reports_missing_auth_packages_from_paths() {
    let customer_root = unique_temp_template_root("customer-root-runtime-missing-auth");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>missing-auth</main></body>
</html>"#,
    );
    fs::write(
        customer_root.join("platform.toml"),
        VALID_CONFIG
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"missing-auth-package\"",
            )
            .replace(
                "enabled = [\"cms-pages\", \"admin-shell\"]",
                "enabled = [\"cms\"]",
            ),
    )
    .unwrap();
    write_customer_root_manifest(&customer_root, "missing-auth-package", &["cms"]);

    let error = match customer_root_runtime_from_paths(&customer_root, "platform.toml") {
        Ok(_) => panic!("expected missing auth package to fail"),
        Err(error) => error,
    };

    fs::remove_dir_all(&customer_root).unwrap();

    assert!(matches!(
        error,
        RuntimeBootstrapError::AuthPackageLoad { package, .. }
            if package == "missing-auth-package"
    ));
}

#[test]
fn direct_builder_bootstraps_customer_root_from_paths() {
    let customer_root = unique_temp_template_root("direct-runtime-builder-bootstrap");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>direct-builder</main></body>
</html>"#,
    );
    fs::write(
        customer_root.join("platform.toml"),
        VALID_CONFIG
            .replace(
                "package = \"platform-default-auth\"",
                "package = \"harbor-auth\"",
            )
            .replace(
                "enabled = [\"cms-pages\", \"admin-shell\"]",
                "enabled = [\"cms\"]",
            ),
    )
    .unwrap();
    write_customer_root_manifest(&customer_root, "harbor-auth", &["cms"]);
    let harbor_auth =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/harbor-shop/auth/harbor-auth");
    copy_directory(&harbor_auth, &customer_root.join("auth/harbor-auth"));

    let plan = Builder::new()
        .register_module(davenda_cms::CmsModule::new())
        .register_customer_plugin(ExampleCheckoutPlugin)
        .build_from_paths(&customer_root, "platform.toml")
        .unwrap();

    fs::remove_dir_all(&customer_root).unwrap();

    assert!(plan.modules.iter().any(|module| module.name == "cms"));
    assert_eq!(plan.auth_package_name, "harbor-auth");
    assert_eq!(plan.linked_customer_plugins.len(), 1);
}

#[test]
fn direct_builder_uses_the_customer_manifest_to_enable_only_selected_modules() {
    let customer_root = unique_temp_template_root("direct-runtime-builder-manifest-enable");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>direct-builder-manifest</main></body>
</html>"#,
    );
    fs::write(
        customer_root.join("platform.toml"),
        VALID_CONFIG.replace(
            "enabled = [\"cms-pages\", \"admin-shell\"]",
            "enabled = [\"cms\"]",
        ),
    )
    .unwrap();
    write_customer_root_manifest(&customer_root, "platform-default-auth", &["cms"]);

    let plan = Builder::new()
        .register_module(davenda_cms::CmsModule::new())
        .register_module(davenda_admin::AdminModule::new())
        .build_from_paths(&customer_root, "platform.toml")
        .unwrap();

    fs::remove_dir_all(&customer_root).unwrap();

    assert_eq!(
        plan.modules
            .iter()
            .map(|module| module.name.as_str())
            .collect::<Vec<_>>(),
        vec!["cms"]
    );
}

#[test]
fn direct_builder_rejects_manifest_modules_not_linked_into_the_customer_binary() {
    let customer_root = unique_temp_template_root("direct-runtime-builder-missing-linked-module");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>missing-linked-module</main></body>
</html>"#,
    );
    fs::write(
        customer_root.join("platform.toml"),
        VALID_CONFIG.replace(
            "enabled = [\"cms-pages\", \"admin-shell\"]",
            "enabled = [\"admin\"]",
        ),
    )
    .unwrap();
    write_customer_root_manifest(&customer_root, "platform-default-auth", &["admin"]);

    let error = Builder::new()
        .register_module(davenda_cms::CmsModule::new())
        .build_from_paths(&customer_root, "platform.toml")
        .unwrap_err();

    fs::remove_dir_all(&customer_root).unwrap();

    assert!(matches!(
        error,
        RuntimeBootstrapError::Build(RuntimeBuildError::CustomerManifestMissingLinkedModules {
            modules
        }) if modules == vec!["admin".to_string()]
    ));
}

#[test]
fn runtime_builder_run_from_env_requires_cookie_secret() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let previous_cookie = std::env::var("DAVENDA_COOKIE_SECRET").ok();
    let previous_csrf = std::env::var("DAVENDA_CSRF_SECRET").ok();
    unsafe {
        std::env::remove_var("DAVENDA_COOKIE_SECRET");
        std::env::remove_var("DAVENDA_CSRF_SECRET");
    }

    let result = RuntimeBuilder::new(config, DefaultAuthModelPackage::default()).run_from_env();

    match previous_cookie {
        Some(value) => unsafe { std::env::set_var("DAVENDA_COOKIE_SECRET", value) },
        None => unsafe { std::env::remove_var("DAVENDA_COOKIE_SECRET") },
    }
    match previous_csrf {
        Some(value) => unsafe { std::env::set_var("DAVENDA_CSRF_SECRET", value) },
        None => unsafe { std::env::remove_var("DAVENDA_CSRF_SECRET") },
    }

    assert!(matches!(
        result,
        Err(RuntimeBootstrapError::MissingEnvironmentVariable {
            name: "DAVENDA_COOKIE_SECRET"
        })
    ));
}

#[test]
fn customer_root_runtime_builder_requires_customer_root_before_build() {
    let config = PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "enabled = [\"cms-pages\", \"admin-shell\"]",
        "enabled = [\"cms\"]",
    ))
    .unwrap();

    let error = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .register_module(davenda_cms::CmsModule::new())
        .build()
        .unwrap_err();

    assert!(matches!(
        error,
        RuntimeBuildError::CustomerRootNotConfigured
    ));
}

#[test]
fn customer_root_runtime_builder_run_from_env_honors_manifest_module_filtering() {
    let customer_root = unique_temp_template_root("customer-root-runtime-run-from-env");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>customer-root-runtime</main></body>
</html>"#,
    );
    write_customer_root_manifest(&customer_root, "platform-default-auth", &["admin"]);
    let config = PlatformConfig::from_toml_str(&VALID_CONFIG.replace(
        "enabled = [\"cms-pages\", \"admin-shell\"]",
        "enabled = [\"admin\"]",
    ))
    .unwrap();

    let error = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .with_customer_root(&customer_root)
        .register_module(davenda_cms::CmsModule::new())
        .run_from_env()
        .unwrap_err();

    fs::remove_dir_all(&customer_root).unwrap();

    assert!(matches!(
        error,
        RuntimeBootstrapError::Build(RuntimeBuildError::CustomerManifestMissingLinkedModules {
            modules
        }) if modules == vec!["admin".to_string()]
    ));
}

#[test]
fn customer_root_runtime_builder_preserves_runtime_builder_escape_hatch() {
    let config = single_node_valid_config();
    let customer_root = unique_temp_template_root("customer-root-builder-escape-hatch");
    write_template_file(
        &customer_root,
        "templates/pages/home.html",
        r#"<!doctype html>
<html xmlns:dv="https://davenda.dev">
  <body><main>customer-root-runtime</main></body>
</html>"#,
    );

    let plan = customer_root_runtime(config, DefaultAuthModelPackage::default())
        .with_customer_root(&customer_root)
        .into_runtime_builder()
        .with_storage_policy_rule(
            PathPolicyRule::new(
                "secure/customer",
                Some(StorageClass::LocalOnlySensitive),
                StoragePolicy::single_node_sensitive(),
            )
            .unwrap()
            .with_local_subdir("customer")
            .unwrap(),
        )
        .build()
        .unwrap();

    fs::remove_dir_all(&customer_root).unwrap();

    let storage_plan = plan
        .storage_host()
        .plan_single_node_escape_hatch_write(StoragePlanRequest::new("secure/customer/runtime.txt"))
        .unwrap();

    assert_eq!(storage_plan.storage_class, StorageClass::LocalOnlySensitive);
    assert_eq!(
        storage_plan.matched_rule_prefix.as_deref(),
        Some("secure/customer")
    );
}

#[test]
fn storage_host_plans_managed_asset_revisions_and_delivery_modes() {
    let config = single_node_valid_config();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_storage_policy_rule(
            PathPolicyRule::new(
                "secure/docs",
                Some(StorageClass::LocalOnlySensitive),
                StoragePolicy::single_node_sensitive(),
            )
            .unwrap()
            .with_local_subdir("vault")
            .unwrap(),
        )
        .build()
        .unwrap();
    let host = plan.storage_host();

    let public_revision = host
        .plan_managed_revision(
            RevisionId::new("rev-public-1").unwrap(),
            "uploads/catalog/brochure.pdf",
            None,
            "application/pdf",
            2_048,
            content_fingerprint('b'),
        )
        .unwrap();
    let mut public_asset = ManagedAsset::new(
        AssetId::new("asset-brochure").unwrap(),
        "Brochure",
        public_revision,
    )
    .unwrap();
    public_asset.publish_current();

    let public_delivery = host.plan_public_asset_delivery(&public_asset).unwrap();
    assert_eq!(public_delivery.delivery_mode(), DeliveryMode::PublicCdn);
    assert_eq!(public_delivery.audience(), DeliveryAudience::Public);
    assert!(matches!(
        public_delivery.target(),
        AssetDeliveryTarget::Cdn { public_url, .. }
            if public_url == "https://cdn.example.com/uploads/catalog/brochure.pdf"
    ));
    assert!(
        public_asset
            .auth_updates()
            .contains(&davenda_auth::DefaultTupleUpdate::Write(
                davenda_auth::DefaultTuple::new(
                    davenda_auth::Entity::asset("asset-brochure"),
                    davenda_auth::Relation::ReadPublic,
                    davenda_auth::DefaultSubject::entity(davenda_auth::Entity::any_user()),
                ),
            ))
    );

    let restricted_revision = host
        .plan_managed_revision_with_single_node_escape_hatch(
            RevisionId::new("rev-secret-1").unwrap(),
            "secure/docs/orders.csv",
            Some(StoragePolicyOverride::force_single_node_escape_hatch()),
            "text/csv",
            256,
            content_fingerprint('c'),
        )
        .unwrap();
    let restricted_asset = ManagedAsset::new(
        AssetId::new("asset-orders").unwrap(),
        "Orders Export",
        restricted_revision,
    )
    .unwrap();

    let authorized_delivery = host
        .plan_authorized_asset_delivery(&restricted_asset)
        .unwrap();
    assert_eq!(authorized_delivery.delivery_mode(), DeliveryMode::LocalOnly);
    assert_eq!(authorized_delivery.audience(), DeliveryAudience::Authorized);
    assert!(matches!(
        authorized_delivery.target(),
        AssetDeliveryTarget::LocalPath { path }
            if path == "/tmp/davenda-runtime-tests/vault/secure/docs/orders.csv"
    ));
}

#[test]
fn search_host_exposes_visibility_and_invalidation_catalog() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .with_module(EventsModule::new())
        .with_module(MediaModule::new())
        .build()
        .unwrap();
    let host = plan.search_host("scheduler-a").unwrap();

    let public_visible = host
        .visible_to(&[])
        .into_iter()
        .map(|index| index.id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    assert!(public_visible.contains("search.cms.pages"));
    assert!(public_visible.contains("search.catalog.products"));
    assert!(public_visible.contains("search.media"));
    assert!(!public_visible.contains("search.events.bookings"));

    let events_visible = host
        .visible_to(&[Capability::EventsBookingCheckIn])
        .into_iter()
        .map(|index| index.id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    assert!(events_visible.contains("search.events.bookings"));

    let published = host.invalidation_plan(SearchInvalidationTrigger::Published);
    let published_ids = published
        .indexes
        .iter()
        .map(|index| index.id.as_str().to_string())
        .collect::<BTreeSet<_>>();
    assert!(published_ids.contains("search.cms.pages"));
    assert!(published_ids.contains("search.catalog.products"));
    assert!(published_ids.contains("search.media"));
    assert!(!published_ids.contains("search.events.bookings"));

    let scheduled = host.scheduled_rebuilds();
    assert!(scheduled.iter().all(|index| matches!(
        index.rebuild_strategy,
        SearchRebuildStrategy::Scheduled { .. }
    )));
}

#[test]
fn search_host_queues_full_reindex_through_ops_bulk_workflow() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(AdminModule::new())
        .with_module(OpsModule::new())
        .with_module(CmsModule::new())
        .with_module(CommerceModule::new())
        .build()
        .unwrap();
    let expected_index_count = plan.ops_catalog.search.contributions.len();

    let mut host = plan.search_host("scheduler-a").unwrap();
    let queued = host
        .queue_full_reindex(
            BulkExecutionId::new("search-reindex-1").unwrap(),
            "operator-1",
            JobInstant::from_unix_seconds(200),
            vec![Capability::SystemModuleManage],
            false,
        )
        .unwrap();

    assert_eq!(queued.plan.definition.id.as_str(), "bulk.search.reindex");
    assert_eq!(queued.plan.target_count, expected_index_count);
    assert_eq!(queued.queued_job_id.as_str(), "search-reindex-1");
}

#[test]
fn runtime_builder_materializes_module_http_surfaces() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(MediaModule::new())
        .build()
        .unwrap();

    let cookie_secret = b"01234567012345670123456701234567";
    let csrf_secret = b"76543210765432107654321076543210";
    let session_cookie = CookieSigner::new(plan.browser.sessions.session_cookie.clone())
        .sign(cookie_secret, "session-456")
        .unwrap();

    let execution = plan
        .execute_request(
            RequestInput::new(HttpMethod::Get, "www.example.com", "/admin/media")
                .unwrap()
                .with_session_cookie(session_cookie)
                .grant_capability(davenda_auth::Capability::AssetRead),
            cookie_secret,
            csrf_secret,
        )
        .unwrap();

    assert_eq!(execution.route.route_name, "media.library");
    assert_eq!(execution.route_area, RouteArea::Admin);
    assert_eq!(
        execution.response,
        HandlerResponse::Page(PageResponse {
            template: "media/library".to_string(),
            status: 200,
        })
    );
}

#[test]
fn runtime_builder_matches_parameterized_module_routes() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(EventsModule::new())
        .build()
        .unwrap();

    let execution = plan
        .execute_request(
            RequestInput::new(
                HttpMethod::Get,
                "www.example.com",
                "/en-GB/events/summer-gala",
            )
            .unwrap(),
            b"01234567012345670123456701234567",
            b"76543210765432107654321076543210",
        )
        .unwrap();

    assert_eq!(execution.route.route_name, "events.detail");
    assert_eq!(execution.locale, "en-GB");
    assert_eq!(
        execution.route.params.get("event_slug").map(String::as_str),
        Some("summer-gala")
    );
    assert_eq!(
        execution.response,
        HandlerResponse::Page(PageResponse {
            template: "events/detail".to_string(),
            status: 200,
        })
    );
}

#[test]
fn http_runtime_generates_named_paths_for_module_routes() {
    let config = PlatformConfig::from_toml_str(VALID_CONFIG).unwrap();
    let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
        .with_module(EventsModule::new())
        .build()
        .unwrap();
    let params = BTreeMap::from([("event_slug".to_string(), "summer-gala".to_string())]);

    let path = plan
        .http
        .path_for(&plan.config, "events.detail", &params, Some("fr-FR"))
        .unwrap();
    assert_eq!(path, "/fr-FR/events/summer-gala");

    let absolute = plan
        .http
        .absolute_url_for(&plan.config, "events.detail", &params, Some("en-GB"))
        .unwrap();
    assert_eq!(absolute, "https://www.example.com/en-GB/events/summer-gala");

    let missing = plan
        .http
        .path_for(
            &plan.config,
            "events.detail",
            &BTreeMap::new(),
            Some("en-GB"),
        )
        .unwrap_err();
    assert_eq!(
        missing,
        RouteUrlError::MissingRouteParameter {
            route: "events.detail".to_string(),
            parameter: "event_slug".to_string(),
        }
    );
}
