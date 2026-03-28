use super::*;

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, CmsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CmsModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, CmsModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(CmsModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn validate_path(field: &'static str, value: String) -> Result<String, CmsModelError> {
    let path = require_non_empty(field, value)?;
    if path.starts_with('/') {
        Ok(path)
    } else {
        Err(CmsModelError::InvalidPath { field, value: path })
    }
}
