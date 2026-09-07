#![forbid(unsafe_code)]

mod i18n;
#[cfg(feature = "site")]
mod islands;
mod model;
mod ui;

pub use i18n::*;
#[cfg(feature = "site")]
pub use islands::*;
pub use model::*;
pub use ui::*;

pub const GITLY_CSS: &str = r#"
:root { color-scheme: light dark; }
body { margin: 0; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }
a { text-underline-offset: .2em; }
"#;
