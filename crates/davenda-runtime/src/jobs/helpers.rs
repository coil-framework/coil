use super::errors::RuntimeJobsError;

pub(crate) fn validate_runtime_identifier(
    field: &'static str,
    value: String,
) -> Result<String, RuntimeJobsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RuntimeJobsError::EmptyValue { field })
    } else {
        Ok(trimmed.to_string())
    }
}
