#![forbid(unsafe_code)]

mod model;
mod state;
mod ui;

pub use model::*;
pub use state::*;
pub use ui::*;

pub const SHOPPR_CSS: &str = include_str!("shoppr.css");
