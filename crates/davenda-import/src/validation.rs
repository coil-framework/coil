use thiserror::Error;

use davenda_report::ReportModelError;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImportModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
    #[error("import manifest declares duplicate importer `{importer_id}`")]
    DuplicateImporter { importer_id: String },
    #[error("importer `{importer_id}` depends on unknown importer `{dependency}`")]
    UnknownImporterDependency {
        importer_id: String,
        dependency: String,
    },
    #[error("importer `{importer_id}` depends on itself")]
    SelfDependency { importer_id: String },
    #[error("import manifest has cyclic importer dependencies")]
    CyclicImporterDependencies,
    #[error("import receipt duplicates source key `{source_key}`")]
    DuplicateSourceReceipt { source_key: String },
    #[error("{0}")]
    Report(#[from] ReportModelError),
}

pub fn require_non_empty(field: &'static str, value: String) -> Result<String, ImportModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ImportModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn validate_token(field: &'static str, value: String) -> Result<String, ImportModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(ImportModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}
