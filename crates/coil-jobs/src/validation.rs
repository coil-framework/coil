use crate::JobsModelError;

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, JobsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(JobsModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(JobsModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, JobsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(JobsModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
