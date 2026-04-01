use std::collections::BTreeSet;

use shoppr_loyalty_backend::service_overview;

#[test]
fn customer_root_workspace_shape_matches_chapter_96_model() {
    let workspace_cargo = include_str!("../../../Cargo.toml");
    let app_cargo = include_str!("../../../crates/shoppr-app/Cargo.toml");
    let backend_cargo = include_str!("../../../crates/shoppr-backend/Cargo.toml");
    let bin_cargo = include_str!("../../../crates/shoppr-bin/Cargo.toml");
    let sidecar_cargo = include_str!("../Cargo.toml");
    let app_manifest = include_str!("../../../app.toml");
    let dev_config = include_str!("../../../platform.dev.toml");

    assert!(workspace_cargo.contains("[workspace]"), "{workspace_cargo}");
    assert!(
        workspace_cargo.contains("\"crates/shoppr-app\""),
        "{workspace_cargo}"
    );
    assert!(
        workspace_cargo.contains("\"crates/shoppr-backend\""),
        "{workspace_cargo}"
    );
    assert!(
        workspace_cargo.contains("\"crates/shoppr-bin\""),
        "{workspace_cargo}"
    );
    assert!(
        workspace_cargo.contains("default-members = [\"crates/shoppr-bin\"]"),
        "{workspace_cargo}"
    );

    assert!(
        app_cargo.contains("shoppr-backend = { path = \"../shoppr-backend\" }"),
        "{app_cargo}"
    );
    assert!(
        bin_cargo.contains("shoppr-app = { path = \"../shoppr-app\" }"),
        "{bin_cargo}"
    );
    assert!(bin_cargo.contains("name = \"shoppr\""), "{bin_cargo}");
    assert!(
        backend_cargo.contains("coil-customer-sdk.workspace = true"),
        "{backend_cargo}"
    );

    assert!(
        sidecar_cargo.contains("name = \"shoppr-loyalty-backend\""),
        "{sidecar_cargo}"
    );
    assert!(sidecar_cargo.contains("publish = false"), "{sidecar_cargo}");
    assert!(
        sidecar_cargo.contains("coil-customer-sdk.workspace = true"),
        "{sidecar_cargo}"
    );
    assert!(
        workspace_cargo.contains("\"backend/shoppr-loyalty-backend\""),
        "{workspace_cargo}"
    );
    assert!(
        !workspace_cargo.contains("[patch.crates-io]"),
        "{workspace_cargo}"
    );
    assert!(
        app_manifest.contains("name = \"shoppr\""),
        "{app_manifest}"
    );
    assert!(
        app_manifest.contains("package = \"shoppr-auth\""),
        "{app_manifest}"
    );
    assert!(
        dev_config.contains("package = \"shoppr-auth\""),
        "{dev_config}"
    );
}

#[test]
fn linked_backend_docs_and_bootstrap_stay_primary() {
    let app_readme = include_str!("../../../README.md");
    let folder_readme = include_str!("../../README.md");
    let crate_readme = include_str!("../README.md");
    let backend_lib = include_str!("../src/lib.rs");
    let backend_http = include_str!("../src/http.rs");
    let app_lib = include_str!("../../../crates/shoppr-app/src/lib.rs");
    let bin_main = include_str!("../../../crates/shoppr-bin/src/main.rs");
    let compose = include_str!("../../../docker-compose.yml");
    let repo_compose = include_str!("../../../docker-compose.repo.yml");

    assert!(
        app_readme.contains("Shoppr As A Customer-Root Workspace"),
        "{app_readme}"
    );
    assert!(app_readme.contains("linked backend crate"), "{app_readme}");
    assert!(
        app_readme.contains("optional sidecar adapter still exists"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("linked customer plugin ids"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("cargo run -p shoppr -- linked-backend demo"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("./scripts/prepare-local-dev.sh"),
        "{app_readme}"
    );
    assert!(
        app_readme.contains(
            "The committed workspace is intentionally free of `patch.crates-io` overlays"
        ),
        "{app_readme}"
    );
    assert!(
        app_readme.contains("uses `apps/shoppr` as the Docker build context"),
        "{app_readme}"
    );
    assert!(
        app_readme
            .contains("docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build"),
        "{app_readme}"
    );

    assert!(
        folder_readme.contains("The primary path here is the chapter 96 model"),
        "{folder_readme}"
    );
    assert!(
        folder_readme.contains("with_customer_plugin(shoppr_loyalty_backend::plugin())"),
        "{folder_readme}"
    );
    assert!(
        folder_readme.contains("admin dashboard renders the linked plugin metadata"),
        "{folder_readme}"
    );
    assert!(
        folder_readme.contains("cargo run -p shoppr -- linked-backend demo"),
        "{folder_readme}"
    );
    assert!(
        folder_readme.contains("./scripts/prepare-local-dev.sh"),
        "{folder_readme}"
    );
    assert!(
        folder_readme.contains("docker compose -f docker-compose.yml -f docker-compose.repo.yml --profile backend-example up --build"),
        "{folder_readme}"
    );
    assert!(
        folder_readme.contains("Optional Sidecar Adapter"),
        "{folder_readme}"
    );

    assert!(
        crate_readme.contains("customer-owned Rust backend crate"),
        "{crate_readme}"
    );
    assert!(
        crate_readme.contains("`plugin()` is the intended registration point"),
        "{crate_readme}"
    );
    assert!(
        crate_readme.contains("cargo run -p shoppr -- linked-backend demo"),
        "{crate_readme}"
    );
    assert!(
        crate_readme.contains("./scripts/prepare-local-dev.sh"),
        "{crate_readme}"
    );
    assert!(
        crate_readme.contains("linked crate first"),
        "{crate_readme}"
    );
    assert!(
        crate_readme.contains("sidecar only when a process"),
        "{crate_readme}"
    );
    assert!(
        crate_readme.contains("Optional Sidecar Adapter"),
        "{crate_readme}"
    );

    assert!(
        backend_lib.contains("Shoppr linked customer backend example"),
        "{backend_lib}"
    );
    assert!(
        backend_lib.contains("with_customer_plugin(shoppr_loyalty_backend::plugin())"),
        "{backend_lib}"
    );
    assert!(
        backend_lib.contains("customer-owned Rust rules to a sidecar process only when"),
        "{backend_lib}"
    );
    assert!(
        backend_http.contains(
            "The primary path for this crate is linking `plugin()` into a customer-owned binary"
        ),
        "{backend_http}"
    );
    assert!(
        app_lib.contains("vec![Box::new(shoppr_backend::plugin())]"),
        "{app_lib}"
    );
    assert!(
        bin_main.contains("Shoppr customer workspace"),
        "{bin_main}"
    );
    assert!(bin_main.contains("linked plugins:"), "{bin_main}");
    assert!(bin_main.contains("LinkedBackendCommand"), "{bin_main}");
    assert!(
        bin_main.contains("linked_backend_demo_output"),
        "{bin_main}"
    );

    assert!(
        compose.contains("profiles: [\"backend-example\"]"),
        "{compose}"
    );
    assert!(compose.contains("context: ."), "{compose}");
    assert!(
        compose.contains("dockerfile: backend/shoppr-loyalty-backend/Dockerfile"),
        "{compose}"
    );
    assert!(
        compose.contains("SHOPPR_BACKEND_BIND: \"0.0.0.0:8091\""),
        "{compose}"
    );
    assert!(
        compose.contains("SHOPPR_BACKEND_WEBHOOK_SECRET"),
        "{compose}"
    );
    assert!(repo_compose.contains("context: ../.."), "{repo_compose}");
    assert!(
        repo_compose.contains("apps/shoppr/Dockerfile.repo"),
        "{repo_compose}"
    );
    assert!(
        repo_compose.contains("apps/shoppr/backend/shoppr-loyalty-backend/Dockerfile.repo"),
        "{repo_compose}"
    );
}

#[test]
fn service_overview_lists_the_example_routes_exactly() {
    let overview = service_overview("Shoppr");
    let endpoints = overview
        .endpoints
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();

    assert_eq!(overview.service, "shoppr-loyalty-backend");
    assert_eq!(overview.brand, "Shoppr");
    assert_eq!(
        endpoints,
        BTreeSet::from([
            "GET /",
            "GET /health",
            "POST /api/loyalty/preview",
            "POST /api/orders/review",
            "POST /webhooks/crm/contact-updated",
        ])
    );
}
