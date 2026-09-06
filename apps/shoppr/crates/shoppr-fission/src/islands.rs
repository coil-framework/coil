use coil::fission::prelude::*;
use coil::fission::site::{BrowserIslandApp, run_browser_island};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
struct BridgeInput<T> {
    #[serde(default)]
    props: T,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SearchIslandProps {
    #[serde(default)]
    products: Vec<SearchProduct>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchProduct {
    pub title: String,
    pub href: String,
}

#[derive(Clone, Debug, Default)]
struct SearchState {
    query: String,
    products: Vec<SearchProduct>,
}

impl GlobalState for SearchState {}

#[fission_reducer(SearchChanged)]
fn search_changed(state: &mut SearchState, ctx: &mut ReducerContext<SearchState>) {
    if let Some(change) = ctx.input.text_change() {
        state.query = change.new_text.clone();
    }
}

#[derive(Clone, Copy)]
struct SearchIsland;

impl From<SearchIsland> for Widget {
    fn from(_: SearchIsland) -> Self {
        let (ctx, view) = coil::fission::build::current::<SearchState>();
        let changed = with_reducer!(ctx, SearchChanged, search_changed);
        let query = view.state().query.trim().to_ascii_lowercase();
        let matches = view
            .state()
            .products
            .iter()
            .filter(|product| {
                query.is_empty() || product.title.to_ascii_lowercase().contains(&query)
            })
            .collect::<Vec<_>>();
        let mut children = vec![
            TextInput {
                id: Some(WidgetId::explicit("shoppr.search.input")),
                semantics_identifier: Some("shoppr.search.input".to_string()),
                label: Some("Search this edit".into()),
                placeholder: Some("Product name".into()),
                value: view.state().query.clone(),
                on_input: Some(changed),
                ..Default::default()
            }
            .into(),
            Text::new(format!("{} pieces", matches.len())).into(),
        ];
        children.extend(
            matches
                .into_iter()
                .map(|product| Link::to(product.title.clone(), product.href.clone()).into()),
        );
        Column {
            gap: Some(12.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

pub fn search_island_boot(input: &str) -> String {
    let props = serde_json::from_str::<BridgeInput<SearchIslandProps>>(input)
        .unwrap_or_default()
        .props;
    run_browser_island("shoppr-search", input, || {
        BrowserIslandApp::new(
            "shoppr-search",
            "shoppr-search",
            SearchState {
                query: String::new(),
                products: props.products,
            },
            SearchIsland,
        )
    })
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CartIslandProps {
    #[serde(default)]
    pub item_count: u32,
    #[serde(default)]
    pub subtotal_minor: i64,
    #[serde(default)]
    pub currency: String,
    pub product_handle: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct CartState {
    item_count: u32,
    subtotal_minor: i64,
    currency: String,
    product_handle: Option<String>,
}

impl GlobalState for CartState {}

#[fission_reducer(StageCartItem)]
fn stage_cart_item(state: &mut CartState) {
    if state.product_handle.is_some() {
        state.item_count = state.item_count.saturating_add(1);
    }
}

#[derive(Clone, Copy)]
struct CartIsland;

impl From<CartIsland> for Widget {
    fn from(_: CartIsland) -> Self {
        let (ctx, view) = coil::fission::build::current::<CartState>();
        let add = with_reducer!(ctx, StageCartItem, stage_cart_item);
        let mut children = vec![
            Text::new("YOUR BAG").weight(700).into(),
            Text::new(format!("{} pieces", view.state().item_count))
                .size(24.0)
                .weight(700)
                .into(),
            Text::new(format!(
                "{} {:.2}",
                view.state().currency,
                view.state().subtotal_minor as f64 / 100.0
            ))
            .into(),
        ];
        if view.state().product_handle.is_some() {
            children.push(
                Button {
                    child: Some(Text::new("Add to bag").into()),
                    on_press: Some(add),
                    ..Default::default()
                }
                .semantics_identifier("shoppr.cart.add")
                .into(),
            );
        }
        Column {
            gap: Some(12.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

pub fn cart_island_boot(input: &str) -> String {
    let props = serde_json::from_str::<BridgeInput<CartIslandProps>>(input)
        .unwrap_or_default()
        .props;
    run_browser_island("shoppr-cart", input, || {
        BrowserIslandApp::new(
            "shoppr-cart",
            "shoppr-cart",
            CartState {
                item_count: props.item_count,
                subtotal_minor: props.subtotal_minor,
                currency: if props.currency.is_empty() {
                    "GBP".to_string()
                } else {
                    props.currency
                },
                product_handle: props.product_handle,
            },
            CartIsland,
        )
    })
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct BookingIslandProps {
    #[serde(default)]
    pub slots: Vec<String>,
}

#[derive(Clone, Debug, Default)]
struct BookingState {
    slots: Vec<String>,
    selected: Option<String>,
}

impl GlobalState for BookingState {}

#[fission_reducer(SelectBookingSlot)]
fn select_booking_slot(state: &mut BookingState, slot: String) {
    if state.slots.contains(&slot) {
        state.selected = Some(slot);
    }
}

#[derive(Clone, Copy)]
struct BookingIsland;

impl From<BookingIsland> for Widget {
    fn from(_: BookingIsland) -> Self {
        let (ctx, view) = coil::fission::build::current::<BookingState>();
        let mut children = vec![Text::new("AVAILABLE SESSIONS").weight(700).into()];
        children.extend(view.state().slots.iter().map(|slot| {
            Button {
                child: Some(Text::new(slot.clone()).into()),
                on_press: Some(ctx.bind(
                    SelectBookingSlot(slot.clone()),
                    reduce_with!(select_booking_slot),
                )),
                ..Default::default()
            }
            .semantics_identifier(format!("shoppr.booking.slot.{slot}"))
            .into()
        }));
        if let Some(selected) = &view.state().selected {
            children.push(
                Text::new(format!("Selected: {selected}"))
                    .weight(700)
                    .into(),
            );
        }
        Column {
            gap: Some(12.0),
            children,
            ..Default::default()
        }
        .into()
    }
}

pub fn booking_island_boot(input: &str) -> String {
    let props = serde_json::from_str::<BridgeInput<BookingIslandProps>>(input)
        .unwrap_or_default()
        .props;
    run_browser_island("shoppr-booking", input, || {
        BrowserIslandApp::new(
            "shoppr-booking",
            "shoppr-booking",
            BookingState {
                slots: props.slots,
                selected: None,
            },
            BookingIsland,
        )
    })
}
