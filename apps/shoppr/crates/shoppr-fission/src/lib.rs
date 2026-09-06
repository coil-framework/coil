#![forbid(unsafe_code)]

#[cfg(feature = "site")]
mod islands;
mod model;
mod state;
mod ui;

#[cfg(feature = "site")]
pub use islands::*;
pub use model::*;
pub use state::*;
pub use ui::*;

pub const SHOPPR_CSS: &str = include_str!("shoppr.css");
