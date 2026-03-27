mod model;
mod module;
#[cfg(test)]
mod tests;

pub use model::*;
pub use module::CmsModule;

pub fn module() -> CmsModule {
    CmsModule::new()
}
