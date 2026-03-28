use crate::A11yError;

pub(crate) fn validate_ratio(field: &'static str, ratio: f32) -> Result<(), A11yError> {
    if ratio.is_finite() && ratio >= 0.0 {
        Ok(())
    } else {
        Err(A11yError::InvalidContrastRatio { field, ratio })
    }
}

pub(crate) fn validate_id(field: &'static str, value: String) -> Result<String, A11yError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(A11yError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(A11yError::InvalidId {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn require_non_empty(field: &'static str, value: String) -> Result<String, A11yError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(A11yError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}
