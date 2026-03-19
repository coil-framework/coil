use crate::error::MediaModelError;

pub(crate) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, MediaModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MediaModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(MediaModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, MediaModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(MediaModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
