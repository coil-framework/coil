use coil::fission::server::{ServerJobRegistry, ServerRenderer, ServerRequest};
use shoppr_app::fission_app::{
    shoppr_server_app, CatalogCollection, CatalogProduct, CatalogResponse, CATALOG_JOB,
};
use std::path::PathBuf;

fn app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn catalog() -> CatalogResponse {
    CatalogResponse {
        products: vec![CatalogProduct {
            id: "product:harbor-cap".to_string(),
            handle: "harbor-cap".to_string(),
            sku: "harbor-cap".to_string(),
            title: "Harbor Cap".to_string(),
            summary: "Canvas, considered for the coast.".to_string(),
            price_minor: 2_900,
            currency: "GBP".to_string(),
            collection_handle: "featured".to_string(),
            inventory_locations: vec!["uk-warehouse".to_string()],
        }],
        collections: vec![CatalogCollection {
            id: "collection:featured".to_string(),
            handle: "featured".to_string(),
            title: "Featured".to_string(),
            label: "The Spring Edit".to_string(),
            summary: "The current Townhouse selection.".to_string(),
        }],
    }
}

fn renderer() -> ServerRenderer {
    let root = app_root();
    let config = coil_config::PlatformConfig::from_file(root.join("platform.dev.toml")).unwrap();
    let jobs = ServerJobRegistry::new().register_job(CATALOG_JOB, |_request, _ctx| Ok(catalog()));
    ServerRenderer::new(shoppr_server_app(root, &config, jobs).unwrap())
}

#[test]
fn public_product_route_is_real_fission_ssr_after_the_catalog_job_settles() {
    let mut request = ServerRequest::get("/en-GB/shop/products/harbor-cap");
    request
        .headers
        .insert("host".to_string(), "uk.localhost:8088".to_string());

    let response = renderer().handle(request).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("lang=\"en-GB\""), "{body}");
    assert!(body.contains("Harbor Cap"), "{body}");
    assert!(body.contains("Canvas, considered for the coast."), "{body}");
    assert!(body.contains("shoppr-cart"), "{body}");
    assert!(body.contains("shoppr-cart.wasm"), "{body}");
    assert!(!body.contains("coil:replace"), "{body}");
}

#[test]
fn locale_is_selected_from_the_site_scoped_route_before_rendering() {
    let mut request = ServerRequest::get("/fr-FR/shop");
    request
        .headers
        .insert("host".to_string(), "fr.localhost:8088".to_string());

    let response = renderer().handle(request).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("lang=\"fr-FR\""), "{body}");
    assert!(body.contains("Nouveautés"), "{body}");
    assert!(
        body.contains("Toute la sélection actuelle, choisie dans nos adresses phares."),
        "{body}"
    );
}

#[test]
fn polish_public_copy_is_rendered_from_the_fission_translation_bundle() {
    let mut request = ServerRequest::get("/pl-PL/shop");
    request
        .headers
        .insert("host".to_string(), "pl.localhost:8088".to_string());

    let response = renderer().handle(request).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("lang=\"pl-PL\""), "{body}");
    assert!(body.contains("Przeszukaj tę kolekcję"), "{body}");
}

#[test]
fn unknown_hosts_fail_closed_before_catalogue_rendering() {
    let mut request = ServerRequest::get("/en-GB/shop");
    request
        .headers
        .insert("host".to_string(), "attacker.example".to_string());

    let error = renderer().handle(request).unwrap_err().to_string();
    assert!(
        error.contains("request locale resolution failed"),
        "{error}"
    );
}

#[test]
fn public_route_inventory_uses_ssr_and_bounded_islands() {
    let routes = renderer().routes();
    let catalog = routes
        .iter()
        .find(|route| route.path == "/:locale/shop")
        .unwrap();
    assert_eq!(catalog.islands.len(), 2);
    let events = routes
        .iter()
        .find(|route| route.path == "/:locale/events")
        .unwrap();
    assert_eq!(events.islands.len(), 1);
    assert!(routes.iter().all(|route| !matches!(
        route.mode,
        coil::fission::server::WebRouteMode::ServerPrivate(_)
            | coil::fission::server::WebRouteMode::ClientApp(_)
    )));
}
