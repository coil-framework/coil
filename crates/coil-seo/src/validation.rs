use crate::SeoError;

pub(crate) fn validate_absolute_url(
    field: &'static str,
    value: String,
) -> Result<String, SeoError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
        Ok(trimmed)
    } else {
        Err(SeoError::InvalidUrl {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn validate_property_name(value: String) -> Result<String, SeoError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '@' | '_' | '-'))
    {
        Err(SeoError::InvalidJsonLdProperty {
            property: trimmed.to_string(),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn require_non_empty(field: &'static str, value: String) -> Result<String, SeoError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(SeoError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
