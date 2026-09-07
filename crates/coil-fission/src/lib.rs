#![forbid(unsafe_code)]

//! The Fission-native application boundary for Coil.
//!
//! Coil supplies product domains and production services. Fission remains the
//! single authority for widgets, routing, reducers, effects, SSR, islands, and
//! full browser applications.

mod access;
mod request;

pub use access::{protected_route_decision, CoilPrincipal, CoilSessionState};
pub use request::{CoilRequestScope, SiteDefinition, SiteRegistry, SiteRegistryError};

#[cfg(feature = "server")]
pub use request::public_revalidation;

pub use fission;
