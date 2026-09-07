use super::model::{
    CART_READ_JOB, CATALOG_JOB, CartRequest, CartSnapshot, CatalogProduct, CatalogRequest,
    CatalogResponse,
};
use super::state::{
    AddCartItem, CartFailed, CartLoaded, CatalogFailed, CatalogLoaded, ShopprState,
    StorefrontRoute, add_cart_item, on_cart_failed, on_cart_loaded, on_catalog_failed,
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
        let cart_loaded = with_reducer!(ctx, CartLoaded, on_cart_loaded);
        let cart_failed = with_reducer!(ctx, CartFailed, on_cart_failed);
        let state = view.state();
        let env = view.env();
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
            if matches!(&state.route, StorefrontRoute::Cart) {
                resources.job(
                    JobResource::new(
                        ResourceKey::new(format!(
                            "shoppr.cart.{}.{}",
                            state.scope.site_id, state.scope.session_id
                        )),
                        CART_READ_JOB,
                        CartRequest {
                            scope: state.scope.clone(),
                        },
                    )
                    .deps((state.scope.site_id.clone(), state.scope.session_id.clone()))
                    .on_ok(cart_loaded)
                    .on_err(cart_failed),
                );
            }
        });

        let tokens = &view.env().theme.tokens;
        Container::new(Column {
            gap: Some(tokens.spacing.xl),
            children: vec![
                site_header(&state.scope.locale, env),
                route_content(state, env),
                site_footer(env),
            ],
            ..Default::default()
        })
        .padding_all(tokens.spacing.xl)
        .into()
    }
}

fn site_header(locale: &str, env: &Env) -> Widget {
    Responsive::new(desktop_site_header(locale, env))
        .case(ResponsiveCase::max_width(
            720.0,
            mobile_site_header(locale, env),
        ))
        .into()
}

fn desktop_site_header(locale: &str, env: &Env) -> Widget {
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
                    Link::to(
                        t(env, "collections.featured_eyebrow", "New arrivals"),
                        format!("/{locale}/shop"),
                    )
                    .into(),
                    Link::to(
                        t(env, "collections.grid_eyebrow", "Edits"),
                        format!("/{locale}/shop/collections"),
                    )
                    .into(),
                    Link::to(
                        t(env, "events.list_eyebrow", "Events"),
                        format!("/{locale}/events"),
                    )
                    .into(),
                    Link::to(t(env, "fission.bag_ready", "Your bag"), "/cart").into(),
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

fn mobile_site_header(locale: &str, env: &Env) -> Widget {
    Column {
        gap: Some(16.0),
        children: vec![
            Text::new("SHOPPR TOWNHOUSE").weight(700).into(),
            Row {
                gap: Some(16.0),
                children: vec![
                    Link::to(
                        t(env, "product.copy.shop", "Shop"),
                        format!("/{locale}/shop"),
                    )
                    .into(),
                    Link::to(
                        t(env, "collections.grid_eyebrow", "Edits"),
                        format!("/{locale}/shop/collections"),
                    )
                    .into(),
                    Link::to(
                        t(env, "events.list_eyebrow", "Events"),
                        format!("/{locale}/events"),
                    )
                    .into(),
                    Link::to(t(env, "fission.bag_ready", "Bag"), "/cart").into(),
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

fn route_content(state: &ShopprState, env: &Env) -> Widget {
    match &state.catalog {
        AsyncSnapshot {
            data: Some(catalog),
            ..
        } => match &state.route {
            StorefrontRoute::Home => home(catalog, &state.scope.locale, env),
            StorefrontRoute::Catalog => catalog_page(catalog, &state.scope.locale, env),
            StorefrontRoute::Collections => collections_page(catalog, &state.scope.locale, env),
            StorefrontRoute::Collection(handle) => {
                collection_page(catalog, handle, &state.scope.locale, env)
            }
            StorefrontRoute::Product(handle) => product_page(catalog, handle, env),
            StorefrontRoute::Events => events_page(env),
            StorefrontRoute::Cart => cart_page(&state.cart, env),
            StorefrontRoute::Account => account_page(env),
            StorefrontRoute::Admin => admin_entry_page(env),
            StorefrontRoute::NotFound => not_found(env),
        },
        AsyncSnapshot {
            error: Some(error), ..
        } => Column {
            gap: Some(12.0),
            children: vec![
                Text::new(t(
                    env,
                    "fission.catalog_unavailable",
                    "The edit is temporarily unavailable",
                ))
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
                Text::new(t(
                    env,
                    "fission.catalog_loading",
                    "Preparing the current edit",
                ))
                .into(),
            ],
            ..Default::default()
        }
        .into(),
    }
}

fn home(catalog: &CatalogResponse, locale: &str, env: &Env) -> Widget {
    Column {
        gap: Some(40.0),
        children: vec![
            Text::new(t(env, "home.hero.title", "THE SPRING EDIT"))
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "home.hero.summary",
                "A considered wardrobe for the city, the coast, and everywhere between.",
            ))
            .size(54.0)
            .weight(700)
            .into(),
            Text::new(t(
                env,
                "fission.home_intro",
                "Natural texture, clean structure, and pieces selected to live with you.",
            ))
            .size(20.0)
            .into(),
            Link::to(
                t(env, "home.hero.secondary_cta", "Explore the campaign"),
                format!("/{locale}/shop"),
            )
            .into(),
            Divider::default().into(),
            Text::new(t(env, "collections.featured_eyebrow", "New arrivals"))
                .size(34.0)
                .weight(700)
                .into(),
            product_grid(&catalog.products, locale, env),
        ],
        ..Default::default()
    }
    .into()
}

fn catalog_page(catalog: &CatalogResponse, locale: &str, env: &Env) -> Widget {
    Column {
        gap: Some(28.0),
        children: vec![
            Text::new(t(env, "collections.featured_eyebrow", "New arrivals"))
                .size(48.0)
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "fission.catalog_intro",
                "The complete current edit, selected across our flagship locations.",
            ))
            .size(19.0)
            .into(),
            search_island_mount(env),
            product_grid(&catalog.products, locale, env),
        ],
        ..Default::default()
    }
    .into()
}

fn collections_page(catalog: &CatalogResponse, locale: &str, env: &Env) -> Widget {
    let mut children = vec![
        Text::new(t(env, "collections_page.title", "Seasonal edits"))
            .size(48.0)
            .weight(700)
            .into(),
    ];
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
                    t(env, "collections.open_edit", "Open the edit"),
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

fn collection_page(catalog: &CatalogResponse, handle: &str, locale: &str, env: &Env) -> Widget {
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
                product_grid(&products, locale, env),
            ],
            ..Default::default()
        }
        .into(),
        None => not_found(env),
    }
}

fn product_page(catalog: &CatalogResponse, handle: &str, env: &Env) -> Widget {
    match catalog.products.iter().find(|item| item.handle == handle) {
        Some(product) => {
            let (ctx, view) = coil::fission::build::current::<ShopprState>();
            let add = ctx.bind(
                AddCartItem(product.handle.clone()),
                reduce_with!(add_cart_item),
            );
            let mut children = vec![
                Text::new(t(env, "fission.current_edit", "SHOPPR / CURRENT EDIT"))
                    .weight(700)
                    .into(),
                Text::new(product.title.clone())
                    .size(52.0)
                    .weight(700)
                    .into(),
                Text::new(money(product)).size(24.0).weight(700).into(),
                Text::new(product.summary.clone()).size(19.0).into(),
                Divider::default().into(),
                Text::new(t(
                    env,
                    "product.accordions.description.title",
                    "Details & care",
                ))
                .size(28.0)
                .weight(700)
                .into(),
                Text::new(format!("Style {} · SKU {}", product.handle, product.sku)).into(),
                Text::new(t(
                    env,
                    "product.accordions.availability.title",
                    "In-store availability",
                ))
                .size(28.0)
                .weight(700)
                .into(),
                Text::new(if product.inventory_locations.is_empty() {
                    t(
                        env,
                        "fission.ask_availability",
                        "Ask the Townhouse team for current availability",
                    )
                } else {
                    product.inventory_locations.join(" · ")
                })
                .into(),
                Button {
                    id: Some(WidgetId::explicit("shoppr.cart.add")),
                    child: Some(Text::new(t(env, "product.copy.add_to_cart", "Add to bag")).into()),
                    on_press: Some(add),
                    ..Default::default()
                }
                .semantics_identifier("shoppr.cart.add")
                .into(),
            ];
            if let Some(cart) = &view.state().cart.data {
                let summary = if cart.item_count == 1 {
                    t(env, "cart.added_singular", "piece now in your bag")
                } else {
                    t(env, "cart.added_plural", "pieces now in your bag")
                };
                children.push(
                    Text::new(format!("{} {summary}", cart.item_count))
                        .weight(700)
                        .into(),
                );
            }
            if let Some(error) = &view.state().cart.error {
                children.push(Text::new(error.message.clone()).into());
            }
            Column {
                gap: Some(22.0),
                children,
                ..Default::default()
            }
            .into()
        }
        None => not_found(env),
    }
}

fn product_grid(products: &[CatalogProduct], locale: &str, env: &Env) -> Widget {
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
                            t(env, "fission.view_piece", "View piece"),
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

fn search_island_mount(env: &Env) -> Widget {
    SemanticsRegion {
        id: Some(WidgetId::explicit("shoppr-search")),
        identifier: Some("shoppr-search".to_string()),
        child: Some(Text::new(t(env, "fission.search_edit", "Search this edit")).into()),
        ..Default::default()
    }
    .into()
}

fn cart_page(
    cart: &AsyncSnapshot<CartSnapshot, super::model::ShopprJobError>,
    env: &Env,
) -> Widget {
    match cart {
        AsyncSnapshot {
            data: Some(cart), ..
        } if cart.lines.is_empty() => EmptyState {
            icon: None,
            title: t(env, "cart.empty_title", "Your bag is empty"),
            description: Some(t(
                env,
                "cart.empty_summary",
                "Explore the current edit and choose a piece to continue.",
            )),
            action: Some(Link::to(t(env, "cart.empty_cta", "Explore the edit"), "/").into()),
        }
        .into(),
        AsyncSnapshot {
            data: Some(cart), ..
        } => Column {
            gap: Some(20.0),
            children: std::iter::once(
                Text::new(t(env, "cart.title", "Your bag"))
                    .size(48.0)
                    .weight(700)
                    .into(),
            )
            .chain(cart.lines.iter().map(|line| {
                Row {
                    gap: Some(20.0),
                    children: vec![
                        Text::new(line.title.clone()).size(22.0).weight(700).into(),
                        Spacer {
                            flex_grow: 1.0,
                            ..Default::default()
                        }
                        .into(),
                        Text::new(format!("{} × {}", line.quantity, line.currency)).into(),
                        Text::new(format!("{:.2}", line.total_minor as f64 / 100.0)).into(),
                    ],
                    ..Default::default()
                }
                .into()
            }))
            .chain(std::iter::once(
                Text::new(format!(
                    "{} {:.2}",
                    cart.currency,
                    cart.subtotal_minor as f64 / 100.0
                ))
                .size(30.0)
                .weight(700)
                .into(),
            ))
            .collect(),
            ..Default::default()
        }
        .into(),
        AsyncSnapshot {
            error: Some(error), ..
        } => Alert {
            kind: AlertKind::Error,
            title: t(env, "cart.error_title", "Your bag is unavailable"),
            description: Some(error.message.clone()),
        }
        .into(),
        _ => Spinner {
            id: WidgetId::explicit("shoppr.cart.loading"),
            color: None,
            motion: Some(SpinnerMotion::Default),
        }
        .into(),
    }
}

fn events_page(env: &Env) -> Widget {
    Column {
        gap: Some(22.0),
        children: vec![
            Text::new(t(env, "events.list_eyebrow", "AT THE TOWNHOUSE"))
                .weight(700)
                .into(),
            Text::new(t(
                env,
                "events.list_title",
                "Events, fittings, and conversations in our spaces.",
            ))
            .size(48.0)
            .weight(700)
            .into(),
            booking_island_mount(env),
        ],
        ..Default::default()
    }
    .into()
}

fn booking_island_mount(env: &Env) -> Widget {
    SemanticsRegion {
        id: Some(WidgetId::explicit("shoppr-booking")),
        identifier: Some("shoppr-booking".to_string()),
        child: Some(Text::new(t(env, "fission.choose_session", "Choose a session")).into()),
        ..Default::default()
    }
    .into()
}

fn account_page(env: &Env) -> Widget {
    Text::new(t(env, "fission.account_title", "Your Shoppr account"))
        .size(48.0)
        .weight(700)
        .into()
}

fn admin_entry_page(env: &Env) -> Widget {
    Text::new(t(env, "fission.admin_title", "Shoppr operations"))
        .size(48.0)
        .weight(700)
        .into()
}

fn not_found(env: &Env) -> Widget {
    Column {
        gap: Some(14.0),
        children: vec![
            Text::new(t(
                env,
                "fission.not_found_title",
                "This page is not in the current edit",
            ))
            .size(40.0)
            .weight(700)
            .into(),
            Link::to(t(env, "fission.not_found_return", "Return to Shoppr"), "/").into(),
        ],
        ..Default::default()
    }
    .into()
}

fn site_footer(env: &Env) -> Widget {
    Column {
        gap: Some(16.0),
        children: vec![
            Divider::default().into(),
            Text::new(t(
                env,
                "home.footer.summary",
                "Shoppr Townhouse · London · Paris · Warsaw",
            ))
            .weight(700)
            .into(),
        ],
        ..Default::default()
    }
    .into()
}

fn t(env: &Env, key: &str, fallback: &str) -> String {
    env.i18n
        .get(&env.locale, key)
        .unwrap_or(fallback)
        .to_string()
}

fn money(product: &CatalogProduct) -> String {
    format!(
        "{} {:.2}",
        product.currency,
        product.price_minor as f64 / 100.0
    )
}
