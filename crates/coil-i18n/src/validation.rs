use crate::I18nError;

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, I18nError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(I18nError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(I18nError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn validate_route(field: &'static str, value: String) -> Result<String, I18nError> {
    let route = require_non_empty(field, value)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(I18nError::InvalidRoute {
            field,
            value: route,
        })
    }
}

pub(crate) fn require_non_empty(field: &'static str, value: String) -> Result<String, I18nError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(I18nError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
