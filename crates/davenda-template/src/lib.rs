use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

mod model;
mod parser;
mod registry;
mod runtime;

pub use model::*;
pub use parser::TemplateSourceParser;
pub use registry::*;
pub use runtime::*;

pub(crate) use runtime::{
    require_non_empty, validate_attribute_name, validate_element_name, validate_token,
};

#[cfg(test)]
mod tests;
