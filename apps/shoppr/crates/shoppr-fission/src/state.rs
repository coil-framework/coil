use super::model::{CATALOG_JOB, CatalogResponse, ShopprJobError};
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
    pub session: CoilSessionState,
}

impl ShopprState {
    pub fn new(scope: CoilRequestScope, route: StorefrontRoute) -> Self {
        Self {
            scope,
            route,
            catalog: AsyncSnapshot::waiting(),
            session: CoilSessionState::SignedOut,
        }
    }
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
