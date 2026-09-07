use coil::fission::server::{ServerJobRegistry, ServerRenderer, ServerRequest};
use shoppr_app::fission_app::{
    ADD_CART_ITEM_JOB, AddCartItem, AddCartItemRequest, CART_READ_JOB, CATALOG_JOB, CartLine,
    CartSnapshot, CatalogCollection, CatalogProduct, CatalogResponse, shoppr_server_app,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
    assert!(body.contains("Add to bag"), "{body}");
    assert!(body.contains("method=\"post\""), "{body}");
    assert!(body.contains("/__fission/action"), "{body}");
    assert!(!body.contains("coil:replace"), "{body}");
}

#[test]
fn add_to_bag_uses_the_server_derived_site_and_session_scope() {
    let root = app_root();
    let config = coil_config::PlatformConfig::from_file(root.join("platform.dev.toml")).unwrap();
    let captured = Arc::new(Mutex::new(None::<AddCartItemRequest>));
    let captured_request = Arc::clone(&captured);
    let jobs = ServerJobRegistry::new()
        .register_job(CATALOG_JOB, |_request, _ctx| Ok(catalog()))
        .register_job(ADD_CART_ITEM_JOB, move |request, _ctx| {
            *captured_request.lock().unwrap() = Some(request);
            Ok(CartSnapshot {
                item_count: 1,
                subtotal_minor: 2_900,
                currency: "GBP".to_string(),
                lines: Vec::new(),
            })
        });
    let renderer = ServerRenderer::new(shoppr_server_app(root, &config, jobs).unwrap());
    let path = "/en-GB/shop/products/harbor-cap";
    let mut initial = ServerRequest::get(path);
    initial
        .headers
        .insert("host".to_string(), "uk.localhost:8088".to_string());
    let initial_response = renderer.handle(initial).unwrap();
    let cookie = initial_response
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, value)| value.split(';').next().unwrap().to_string())
        .unwrap();
    let token = renderer.sign_action(
        path,
        coil::fission::prelude::WidgetId::explicit("shoppr.cart.add").as_u128(),
        AddCartItem("harbor-cap".to_string()),
        Duration::from_secs(60),
    );
    let mut action = ServerRequest::post("/__fission/action", serde_json::to_vec(&token).unwrap());
    action
        .headers
        .insert("host".to_string(), "uk.localhost:8088".to_string());
    action.headers.insert("cookie".to_string(), cookie);

    let response = renderer.handle(action).unwrap();
    let request = captured.lock().unwrap().clone().unwrap();

    assert_eq!(response.status, 200);
    assert!(response.body_string().contains("1 piece now in your bag"));
    assert_eq!(request.scope.site_id, "shoppr-uk");
    assert!(!request.scope.session_id.is_empty());
    assert_eq!(request.product_handle, "harbor-cap");
    assert_eq!(request.quantity, 1);
}

#[test]
fn cart_route_waits_for_the_session_cart_job_before_rendering() {
    let root = app_root();
    let config = coil_config::PlatformConfig::from_file(root.join("platform.dev.toml")).unwrap();
    let jobs = ServerJobRegistry::new()
        .register_job(CATALOG_JOB, |_request, _ctx| Ok(catalog()))
        .register_job(CART_READ_JOB, |request, _ctx| {
            assert_eq!(request.scope.site_id, "shoppr-uk");
            assert!(!request.scope.session_id.is_empty());
            Ok(CartSnapshot {
                item_count: 2,
                subtotal_minor: 5_800,
                currency: "GBP".to_string(),
                lines: vec![CartLine {
                    product_id: "product:harbor-cap".to_string(),
                    product_handle: "harbor-cap".to_string(),
                    title: "Harbor Cap".to_string(),
                    quantity: 2,
                    unit_price_minor: 2_900,
                    total_minor: 5_800,
                    currency: "GBP".to_string(),
                }],
            })
        });
    let renderer = ServerRenderer::new(shoppr_server_app(root, &config, jobs).unwrap());
    let mut request = ServerRequest::get("/cart");
    request
        .headers
        .insert("host".to_string(), "uk.localhost:8088".to_string());

    let response = renderer.handle(request).unwrap();
    let body = response.body_string();

    assert_eq!(response.status, 200);
    assert!(body.contains("Harbor Cap"), "{body}");
    assert!(body.contains("GBP 58.00"), "{body}");
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
    assert_eq!(catalog.islands.len(), 1);
    let events = routes
        .iter()
        .find(|route| route.path == "/:locale/events")
        .unwrap();
    assert_eq!(events.islands.len(), 1);
    let cart = routes.iter().find(|route| route.path == "/cart").unwrap();
    assert!(matches!(
        cart.mode,
        coil::fission::server::WebRouteMode::ServerPrivate(_)
    ));
    assert!(
        routes
            .iter()
            .filter(|route| route.path != "/cart")
            .all(|route| {
                !matches!(
                    route.mode,
                    coil::fission::server::WebRouteMode::ServerPrivate(_)
                        | coil::fission::server::WebRouteMode::ClientApp(_)
                )
            })
    );
}
