use super::*;

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppModelError::EmptyField { field });
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        Ok(trimmed.to_string())
    } else {
        Err(AppModelError::InvalidToken {
            field,
            value: trimmed.to_string(),
        })
    }
}

pub(crate) fn validate_hostname(
    field: &'static str,
    value: String,
) -> Result<String, AppModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.'))
        && trimmed.contains('.')
    {
        Ok(trimmed)
    } else {
        Err(AppModelError::InvalidHostname {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, AppModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AppModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_sha256(field: &'static str, value: String) -> Result<String, AppModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed.len() == 64
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        Ok(trimmed)
    } else {
        Err(AppModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn sorted_strings(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

pub(crate) fn sorted_locale_strings(locales: &[LocaleTag]) -> Vec<String> {
    sorted_strings(locales.iter().map(ToString::to_string).collect())
}

pub(crate) fn difference(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|value| !right.contains(value))
        .cloned()
        .collect()
}

pub(crate) fn join_display<T>(values: impl IntoIterator<Item = T>) -> String
where
    T: fmt::Display,
{
    let rendered = values
        .into_iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    if rendered.is_empty() {
        "none".to_string()
    } else {
        rendered.join(",")
    }
}
