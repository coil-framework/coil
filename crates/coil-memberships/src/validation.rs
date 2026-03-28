use crate::MembershipModelError;

pub(crate) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, MembershipModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MembershipModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(MembershipModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, MembershipModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(MembershipModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn ensure_positive_quantity(
    field: &'static str,
    quantity: u32,
) -> Result<(), MembershipModelError> {
    if quantity == 0 {
        Err(MembershipModelError::InvalidQuantity { field, quantity })
    } else {
        Ok(())
    }
}
