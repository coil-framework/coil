use super::model::{
    ADD_CART_ITEM_JOB, AddCartItemRequest, CART_READ_JOB, CATALOG_JOB, CartSnapshot,
    CatalogResponse, ShopprJobError,
};
use coil::fission::prelude::*;
use coil::{CoilRequestScope, CoilSessionState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorefrontRoute {
    Home,
    Catalog,
    Collections,
    Collection(String),
    Product(String),
    Events,
    Cart,
    Account,
    Admin,
    NotFound,
}

impl StorefrontRoute {
    pub fn collection(&self) -> Option<String> {
        match self {
            Self::Collection(handle) => Some(handle.clone()),
            _ => None,
        }
    }

    pub fn product(&self) -> Option<String> {
        match self {
            Self::Product(handle) => Some(handle.clone()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ShopprState {
    pub scope: CoilRequestScope,
    pub route: StorefrontRoute,
    pub catalog: AsyncSnapshot<CatalogResponse, ShopprJobError>,
    pub cart: AsyncSnapshot<CartSnapshot, ShopprJobError>,
    pub session: CoilSessionState,
}

impl ShopprState {
    pub fn new(scope: CoilRequestScope, route: StorefrontRoute) -> Self {
        Self {
            scope,
            route,
            catalog: AsyncSnapshot::waiting(),
            cart: AsyncSnapshot::waiting(),
            session: CoilSessionState::SignedOut,
        }
    }
}

#[fission_reducer(CartLoaded)]
pub fn on_cart_loaded(state: &mut ShopprState, ctx: &mut ReducerContext<ShopprState>) {
    let cart = ctx
        .input
        .job_ok(ADD_CART_ITEM_JOB)
        .or_else(|| ctx.input.job_ok(CART_READ_JOB));
    if let Some(cart) = cart {
        state.cart = AsyncSnapshot::with_data(AsyncConnectionState::Done, cart);
    }
}

#[fission_reducer(CartFailed)]
pub fn on_cart_failed(state: &mut ShopprState, ctx: &mut ReducerContext<ShopprState>) {
    let error = ctx
        .input
        .job_err(ADD_CART_ITEM_JOB)
        .or_else(|| ctx.input.job_err(CART_READ_JOB))
        .unwrap_or_else(|| ShopprJobError::unavailable("The bag could not be loaded"));
    state.cart = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}

#[fission_reducer(AddCartItem)]
pub fn add_cart_item(
    state: &mut ShopprState,
    handle: String,
    ctx: &mut ReducerContext<ShopprState>,
) {
    state.cart = AsyncSnapshot::waiting();
    let loaded = ctx.effects.bind(CartLoaded, on_cart_loaded);
    let failed = ctx.effects.bind(CartFailed, on_cart_failed);
    ctx.effects
        .app(
            ADD_CART_ITEM_JOB,
            AddCartItemRequest {
                scope: state.scope.clone(),
                product_handle: handle,
                quantity: 1,
            },
        )
        .on_ok(loaded)
        .on_err(failed)
        .dispatch();
}

impl GlobalState for ShopprState {}

#[fission_reducer(CatalogLoaded)]
pub fn on_catalog_loaded(state: &mut ShopprState, ctx: &mut ReducerContext<ShopprState>) {
    if let Some(catalog) = ctx.input.job_ok(CATALOG_JOB) {
        state.catalog = AsyncSnapshot::with_data(AsyncConnectionState::Done, catalog);
    }
}

#[fission_reducer(CatalogFailed)]
pub fn on_catalog_failed(state: &mut ShopprState, ctx: &mut ReducerContext<ShopprState>) {
    let error = ctx.input.job_err(CATALOG_JOB).unwrap_or_else(|| {
        ShopprJobError::unavailable(
            ctx.input
                .job_error_message(CATALOG_JOB)
                .unwrap_or("The catalogue could not be loaded"),
        )
    });
    state.catalog = AsyncSnapshot::with_error(AsyncConnectionState::Done, error);
}
