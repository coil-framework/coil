mod error;
mod ids;
mod model;
mod module;
mod validation;

pub use error::*;
pub use ids::*;
pub use model::*;
pub use module::AdminModule;

pub fn module() -> AdminModule {
    AdminModule::new()
}

#[cfg(test)]
mod tests;
