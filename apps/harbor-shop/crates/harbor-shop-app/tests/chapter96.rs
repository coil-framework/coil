use davenda_all::CustomerBackendPlugin;

#[test]
fn linked_customer_backend_descriptor_stays_stable() {
    let descriptor = harbor_shop_backend::plugin().descriptor();

    assert_eq!(descriptor.id, "harbor-shop-backend");
    assert_eq!(descriptor.display_name, "Harbor Shop Linked Backend");
}

#[test]
fn admin_dashboard_surfaces_the_linked_workspace_backend() {
    let dashboard = include_str!("../../../templates/admin/dashboard.html");
    let app_readme = include_str!("../../../README.md");
    let cargo_toml = include_str!("../../../Cargo.toml");

    assert!(dashboard.contains("Linked customer backend"), "{dashboard}");
    assert!(dashboard.contains("linkedCustomerPlugins"), "{dashboard}");
    assert!(
        dashboard.contains("Workspace-owned Rust hook path"),
        "{dashboard}"
    );

    assert!(
        app_readme.contains("cargo run -p harbor-shop -- describe"),
        "{app_readme}"
    );
    assert!(app_readme.contains("patch.crates-io"), "{app_readme}");
    assert!(
        app_readme.contains("Harbor Shop Linked Backend"),
        "{app_readme}"
    );
    assert!(cargo_toml.contains("[patch.crates-io]"), "{cargo_toml}");
    assert!(cargo_toml.contains("davenda-all = \"0.1.0\""), "{cargo_toml}");
}
