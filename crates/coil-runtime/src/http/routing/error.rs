use thiserror::Error;

use super::HttpMethod;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteBuildError {
    #[error("route names must not be empty")]
    EmptyRouteName,
    #[error("route paths must start with `/`, got `{path}`")]
    InvalidRoutePath { path: String },
    #[error("host pattern must not be empty")]
    EmptyHostPattern,
    #[error("route `{name}` is registered more than once for method {method:?}")]
    DuplicateRoute { name: String, method: HttpMethod },
    #[error(
        "route `{name}` requires capability `{capability}` but the auth package does not bind it"
    )]
    MissingCapabilityBinding {
        name: String,
        capability: coil_auth::Capability,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteUrlError {
    #[error("route `{route}` is not registered")]
    UnknownRoute { route: String },
    #[error("route `{route}` requires parameter `{parameter}`")]
    MissingRouteParameter { route: String, parameter: String },
    #[error("route `{route}` does not support locale `{locale}`")]
    UnsupportedLocale { route: String, locale: String },
}

pub(crate) fn validate_route_name(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyRouteName)
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_route_path(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.starts_with('/') && !trimmed.is_empty() {
        Ok(trimmed.to_string())
    } else {
        Err(RouteBuildError::InvalidRoutePath {
            path: value.trim().to_string(),
        })
    }
}

pub(crate) fn validate_host(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyHostPattern)
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_template_name(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}

pub(crate) fn validate_fragment_id(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}
