use super::model::{CATALOG_JOB, CatalogProduct, CatalogRequest, CatalogResponse};
use super::state::{
    CatalogFailed, CatalogLoaded, ShopprState, StorefrontRoute, on_catalog_failed,
    on_catalog_loaded,
};
use coil::fission::core::{JobResource, ResourceKey};
use coil::fission::prelude::*;

#[derive(Clone, Copy)]
pub struct StorefrontPage;

impl From<StorefrontPage> for Widget {
    fn from(_: StorefrontPage) -> Self {
        let (ctx, view) = coil::fission::build::current::<ShopprState>();
        let loaded = with_reducer!(ctx, CatalogLoaded, on_catalog_loaded);
        let failed = with_reducer!(ctx, CatalogFailed, on_catalog_failed);
        let state = view.state();
        let request = CatalogRequest {
            scope: state.scope.clone(),
            collection: state.route.collection(),
            product: state.route.product(),
            search: None,
        };
        ctx.with_resources(|resources| {
            resources.job(
                JobResource::new(
                    ResourceKey::new(format!(
                        "shoppr.catalog.{}.{}",
                        request.scope.site_id, request.scope.route
                    )),
                    CATALOG_JOB,
                    request.clone(),
                )
                .deps(request)
                .on_ok(loaded)
                .on_err(failed),
            );
        });

        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            gap: Some(tokens.spacing.xl),
            children: vec![
                site_header(&state.scope.locale),
                route_content(state),
                site_footer(),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .into()
    }
}

fn site_header(locale: &str) -> Widget {
    Responsive::new(desktop_site_header(locale))
        .case(ResponsiveCase::max_width(720.0, mobile_site_header(locale)))
        .into()
}

fn desktop_site_header(locale: &str) -> Widget {
    Column {
        gap: Some(18.0),
        children: vec![
            Row {
                gap: Some(24.0),
                children: vec![
                    Text::new("SHOPPR TOWNHOUSE").weight(700).into(),
                    Spacer {
                        flex_grow: 1.0,
                        ..Default::default()
                    }
                    .into(),
                    Link::to("New arrivals", format!("/{locale}/shop")).into(),
                    Link::to("Edits", format!("/{locale}/shop/collections")).into(),
                    Link::to("Events", format!("/{locale}/events")).into(),
                    Link::to("Account", format!("/{locale}/account")).into(),
                ],
                ..Default::default()
            }
            .into(),
            Divider::default().into(),
        ],
        ..Default::default()
    }
    .into()
}

fn mobile_site_header(locale: &str) -> Widget {
    Column {
        gap: Some(16.0),
        children: vec![
            Text::new("SHOPPR TOWNHOUSE").weight(700).into(),
            Row {
                gap: Some(16.0),
                children: vec![
                    Link::to("Shop", format!("/{locale}/shop")).into(),
                    Link::to("Edits", format!("/{locale}/shop/collections")).into(),
                    Link::to("Events", format!("/{locale}/events")).into(),
                    Link::to("Account", format!("/{locale}/account")).into(),
                ],
                ..Default::default()
            }
            .into(),
            Divider::default().into(),
        ],
        ..Default::default()
    }
    .into()
}

fn route_content(state: &ShopprState) -> Widget {
    match &state.catalog {
        AsyncSnapshot {
            data: Some(catalog),
            ..
        } => match &state.route {
            StorefrontRoute::Home => home(catalog, &state.scope.locale),
            StorefrontRoute::Catalog => catalog_page(catalog, &state.scope.locale),
            StorefrontRoute::Collections => collections_page(catalog, &state.scope.locale),
            StorefrontRoute::Collection(handle) => {
                collection_page(catalog, handle, &state.scope.locale)
            }
            StorefrontRoute::Product(handle) => product_page(catalog, handle),
            StorefrontRoute::Events => events_page(),
            StorefrontRoute::Account => account_page(),
            StorefrontRoute::Admin => admin_entry_page(),
            StorefrontRoute::NotFound => not_found(),
        },
        AsyncSnapshot {
            error: Some(error), ..
        } => Column {
            gap: Some(12.0),
            children: vec![
                Text::new("The edit is temporarily unavailable")
                    .size(34.0)
                    .weight(700)
                    .into(),
                Text::new(error.message.clone()).into(),
            ],
            ..Default::default()
        }
        .into(),
        _ => Column {
            gap: Some(16.0),
            children: vec![
                Spinner {
                    id: WidgetId::explicit("shoppr.catalog.loading"),
                    color: None,
                    motion: Some(SpinnerMotion::Default),
                }
                .into(),
                Text::new("Preparing the current edit").into(),
            ],
            ..Default::default()
        }
        .into(),
    }
}

fn home(catalog: &CatalogResponse, locale: &str) -> Widget {
    Column {
        gap: Some(40.0),
        children: vec![
            Text::new("THE SPRING EDIT").weight(700).into(),
            Text::new("A considered wardrobe for the city, the coast, and everywhere between.")
                .size(54.0)
                .weight(700)
                .into(),
            Text::new("Natural texture, clean structure, and pieces selected to live with you.")
                .size(20.0)
                .into(),
            Link::to("Explore the campaign", format!("/{locale}/shop")).into(),
            Divider::default().into(),
            Text::new("New arrivals").size(34.0).weight(700).into(),
            product_grid(&catalog.products, locale),
        ],
        ..Default::default()
    }
    .into()
}

fn catalog_page(catalog: &CatalogResponse, locale: &str) -> Widget {
    Column {
        gap: Some(28.0),
        children: vec![
            Text::new("New arrivals").size(48.0).weight(700).into(),
            Text::new("The complete current edit, selected across our flagship locations.")
                .size(19.0)
                .into(),
            search_island_mount(),
            product_grid(&catalog.products, locale),
            cart_island_mount(),
        ],
        ..Default::default()
    }
    .into()
}

fn collections_page(catalog: &CatalogResponse, locale: &str) -> Widget {
    let mut children = vec![Text::new("Seasonal edits").size(48.0).weight(700).into()];
    children.extend(catalog.collections.iter().map(|collection| {
        Column {
            gap: Some(8.0),
            children: vec![
                Text::new(collection.label.clone()).weight(700).into(),
                Text::new(collection.title.clone())
                    .size(30.0)
                    .weight(700)
                    .into(),
                Text::new(collection.summary.clone()).into(),
                Link::to(
                    "Open the edit",
                    format!("/{locale}/shop/collections/{}", collection.handle),
                )
                .into(),
                Divider::default().into(),
            ],
            ..Default::default()
        }
        .into()
    }));
    Column {
        gap: Some(24.0),
        children,
        ..Default::default()
    }
    .into()
}

fn collection_page(catalog: &CatalogResponse, handle: &str, locale: &str) -> Widget {
    let collection = catalog
        .collections
        .iter()
        .find(|item| item.handle == handle);
    let products = catalog
        .products
        .iter()
        .filter(|product| product.collection_handle == handle)
        .cloned()
        .collect::<Vec<_>>();
    match collection {
        Some(collection) => Column {
            gap: Some(28.0),
            children: vec![
                Text::new(collection.label.clone()).weight(700).into(),
                Text::new(collection.title.clone())
                    .size(48.0)
                    .weight(700)
                    .into(),
                Text::new(collection.summary.clone()).size(19.0).into(),
                product_grid(&products, locale),
            ],
            ..Default::default()
        }
        .into(),
        None => not_found(),
    }
}

fn product_page(catalog: &CatalogResponse, handle: &str) -> Widget {
    match catalog.products.iter().find(|item| item.handle == handle) {
        Some(product) => Column {
            gap: Some(22.0),
            children: vec![
                Text::new("SHOPPR / CURRENT EDIT").weight(700).into(),
                Text::new(product.title.clone())
                    .size(52.0)
                    .weight(700)
                    .into(),
                Text::new(money(product)).size(24.0).weight(700).into(),
                Text::new(product.summary.clone()).size(19.0).into(),
                Divider::default().into(),
                Text::new("Details & care").size(28.0).weight(700).into(),
                Text::new(format!("Style {} · SKU {}", product.handle, product.sku)).into(),
                Text::new("In-store availability")
                    .size(28.0)
                    .weight(700)
                    .into(),
                Text::new(if product.inventory_locations.is_empty() {
                    "Ask the Townhouse team for current availability".to_string()
                } else {
                    product.inventory_locations.join(" · ")
                })
                .into(),
                cart_island_mount(),
            ],
            ..Default::default()
        }
        .into(),
        None => not_found(),
    }
}

fn product_grid(products: &[CatalogProduct], locale: &str) -> Widget {
    SimpleGrid {
        min_child_width: 260.0,
        gap: Some(28.0),
        children: products
            .iter()
            .map(|product| {
                Column {
                    gap: Some(10.0),
                    children: vec![
                        Text::new(product.collection_handle.to_uppercase())
                            .weight(700)
                            .into(),
                        Text::new(product.title.clone())
                            .size(26.0)
                            .weight(700)
                            .into(),
                        Text::new(product.summary.clone()).into(),
                        Text::new(money(product)).weight(700).into(),
                        Link::to(
                            "View piece",
                            format!("/{locale}/shop/products/{}", product.handle),
                        )
                        .into(),
                        Divider::default().into(),
                    ],
                    ..Default::default()
                }
                .into()
            })
            .collect(),
    }
    .into()
}

fn search_island_mount() -> Widget {
    SemanticsRegion {
        id: Some(WidgetId::explicit("shoppr-search")),
        identifier: Some("shoppr-search".to_string()),
        child: Some(Text::new("Search this edit").into()),
        ..Default::default()
    }
    .into()
}

fn cart_island_mount() -> Widget {
    SemanticsRegion {
        id: Some(WidgetId::explicit("shoppr-cart")),
        identifier: Some("shoppr-cart".to_string()),
        child: Some(Text::new("Your bag is ready").into()),
        ..Default::default()
    }
    .into()
}

fn events_page() -> Widget {
    Column {
        gap: Some(22.0),
        children: vec![
            Text::new("AT THE TOWNHOUSE").weight(700).into(),
            Text::new("Events, fittings, and conversations in our spaces.")
                .size(48.0)
                .weight(700)
                .into(),
            booking_island_mount(),
        ],
        ..Default::default()
    }
    .into()
}

fn booking_island_mount() -> Widget {
    SemanticsRegion {
        id: Some(WidgetId::explicit("shoppr-booking")),
        identifier: Some("shoppr-booking".to_string()),
        child: Some(Text::new("Choose a session").into()),
        ..Default::default()
    }
    .into()
}

fn account_page() -> Widget {
    Text::new("Your Shoppr account")
        .size(48.0)
        .weight(700)
        .into()
}

fn admin_entry_page() -> Widget {
    Text::new("Shoppr operations").size(48.0).weight(700).into()
}

fn not_found() -> Widget {
    Column {
        gap: Some(14.0),
        children: vec![
            Text::new("This page is not in the current edit")
                .size(40.0)
                .weight(700)
                .into(),
            Link::to("Return to Shoppr", "/").into(),
        ],
        ..Default::default()
    }
    .into()
}

fn site_footer() -> Widget {
    Column {
        gap: Some(16.0),
        children: vec![
            Divider::default().into(),
            Text::new("Shoppr Townhouse · London · Paris · Warsaw")
                .weight(700)
                .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn money(product: &CatalogProduct) -> String {
    format!(
        "{} {:.2}",
        product.currency,
        product.price_minor as f64 / 100.0
    )
}
