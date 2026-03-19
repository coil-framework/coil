use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use zanzibar::{
    CheckRequest, NamespaceConfig, Object, RebacEngine, RebacError, RelationRule, Schema,
    SchemaBuilder, Subject, Tuple, TupleUpdate,
};

mod capability;
mod explain;
mod schema;
mod service;
mod types;

pub use capability::*;
pub use explain::*;
pub use schema::*;
pub use service::*;
pub use types::*;

pub(crate) use explain::build_capability_explanation;

#[cfg(test)]
mod tests;
