use super::*;

pub(super) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, EventModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(EventModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

pub(super) fn validate_route(
    field: &'static str,
    value: String,
) -> Result<String, EventModelError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(EventModelError::InvalidRoute {
            field,
            value: route,
        })
    }
}

pub(super) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, EventModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(EventModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
