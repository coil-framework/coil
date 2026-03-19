use crate::CommerceModelError;

pub(crate) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, CommerceModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(CommerceModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, CommerceModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CommerceModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
