use crate::AssetModelError;

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, AssetModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(AssetModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn normalize_manifest_path(
    field: &'static str,
    value: String,
) -> Result<String, AssetModelError> {
    let value = require_non_empty(field, value)?;
    let normalized = value.trim_matches('/').to_string();
    if normalized.is_empty() {
        Err(AssetModelError::EmptyField { field })
    } else {
        Ok(normalized)
    }
}

pub(crate) fn join_delivery_base(base: &str, suffix: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        suffix.trim_start_matches('/')
    )
}
