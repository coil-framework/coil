use crate::error::AdminModelError;

pub(crate) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, AdminModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AdminModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(AdminModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn validate_route(
    field: &'static str,
    value: String,
) -> Result<String, AdminModelError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(AdminModelError::InvalidRoute {
            field,
            value: route,
        })
    }
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, AdminModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AdminModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
