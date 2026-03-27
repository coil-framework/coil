#[test]
fn harbor_readme_makes_wasm_extensions_concrete_and_bounded() {
    let app_readme = include_str!("../../../README.md");
    let extensions_readme = include_str!("../../../extensions/README.md");
    let waitlist_readme = include_str!("../../../extensions/harbor-waitlist-tools/README.md");
    let package = include_str!("../../../extensions/harbor-waitlist-tools/package.example.toml");
    let config_schema =
        include_str!("../../../extensions/harbor-waitlist-tools/config-schema.example.toml");
    let app_manifest = include_str!("../../../app.toml");

    assert!(
        app_readme.contains("harbor-waitlist-tools"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("runtime-installed"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("linked customer Rust crate"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("extensions/harbor-waitlist-tools/"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("not installed by default"),
        "{app_readme}"
    );

    assert!(
        extensions_readme.contains("linked Rust is the primary path"),
        "{extensions_readme}"
    );
    assert!(
        extensions_readme.contains("WASM remains the bounded path"),
        "{extensions_readme}"
    );

    assert!(
        waitlist_readme.contains("runtime-installed rather than linked into the Harbor customer binary"),
        "{waitlist_readme}"
    );
    assert!(
        waitlist_readme.contains("admin widget"),
        "{waitlist_readme}"
    );
    assert!(
        waitlist_readme.contains("scheduled reconciliation job")
            || waitlist_readme.contains("scheduled reconciliation"),
        "{waitlist_readme}"
    );

    assert!(
        package.contains("id = \"harbor-waitlist-tools\""),
        "{package}"
    );
    assert!(
        package.contains("point = \"admin-widget\""),
        "{package}"
    );
    assert!(
        package.contains("point = \"scheduled-job\""),
        "{package}"
    );
    assert!(
        package.contains("auth-check"),
        "{package}"
    );
    assert!(
        package.contains("outbound-http:partner_crm"),
        "{package}"
    );

    assert!(
        config_schema.contains("partner_crm_integration"),
        "{config_schema}"
    );
    assert!(
        config_schema.contains("show_exception_only"),
        "{config_schema}"
    );

    assert!(
        !app_manifest.contains("harbor-waitlist-tools"),
        "{app_manifest}"
    );
}
