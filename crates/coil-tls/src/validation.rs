use crate::TlsModelError;

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, TlsModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(TlsModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '*'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(TlsModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}
