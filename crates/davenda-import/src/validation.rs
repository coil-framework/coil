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
    #[error("failed to read import manifest `{path}`: {message}")]
    ManifestRead { path: String, message: String },
    #[error("failed to parse import manifest: {message}")]
    ManifestParse { message: String },
    #[error("import manifest references missing `{field}` path `{path}`")]
    ManifestReferenceMissing { field: &'static str, path: String },
    #[error("import manifest field `{field}` is invalid: {message}")]
    InvalidManifestContract {
        field: &'static str,
        message: String,
    },
    #[error("import manifest declares duplicate source input `{input_id}`")]
    DuplicateSourceInput { input_id: String },
    #[error("importer `{importer_id}` is missing a source path")]
    MissingImporterSourcePath { importer_id: String },
    #[error("failed to read import source `{path}` for importer `{importer_id}`: {message}")]
    SourceRead {
        importer_id: String,
        path: String,
        message: String,
    },
    #[error("failed to parse import source `{path}` for importer `{importer_id}`: {message}")]
    SourceParse {
        importer_id: String,
        path: String,
        message: String,
    },
    #[error(
        "import source `{path}` for importer `{importer_id}` must contain a JSON array or an object with a `records` array"
    )]
    SourceShape { importer_id: String, path: String },
    #[error("importer `{importer_id}` has unsupported source format `{source_format}`")]
    UnsupportedSourceFormat {
        importer_id: String,
        source_format: String,
    },
    #[error("importer `{importer_id}` has unsupported resource kind `{resource_kind}`")]
    UnsupportedResourceKind {
        importer_id: String,
        resource_kind: String,
    },
    #[error("importer `{importer_id}` record `{record}` is invalid: {message}")]
    InvalidSourceRecord {
        importer_id: String,
        record: String,
        message: String,
    },
    #[error("failed to persist import artifact `{path}`: {message}")]
    ArtifactWrite { path: String, message: String },
    #[error(
        "import execution hook failed for importer `{importer_id}` record `{record}`: {message}"
    )]
    ExecutionHook {
        importer_id: String,
        record: String,
        message: String,
    },
    #[error("failed to read import journal `{path}`: {message}")]
    JournalRead { path: String, message: String },
    #[error("failed to write import journal `{path}`: {message}")]
    JournalWrite { path: String, message: String },
    #[error("failed to parse import journal `{path}`: {message}")]
    JournalParse { path: String, message: String },
    #[error(
        "import journal `{path}` belongs to run `{actual_run_id}` for customer app `{actual_customer_app_id}`, not `{expected_run_id}` / `{expected_customer_app_id}`"
    )]
    JournalRunMismatch {
        path: String,
        expected_run_id: String,
        actual_run_id: String,
        expected_customer_app_id: String,
        actual_customer_app_id: String,
    },
    #[error("failed to read cutover journal `{path}`: {message}")]
    CutoverJournalRead { path: String, message: String },
    #[error("failed to write cutover journal `{path}`: {message}")]
    CutoverJournalWrite { path: String, message: String },
    #[error("failed to parse cutover journal `{path}`: {message}")]
    CutoverJournalParse { path: String, message: String },
    #[error(
        "cutover journal `{path}` belongs to run `{actual_run_id}` for customer app `{actual_customer_app_id}`, not `{expected_run_id}` / `{expected_customer_app_id}`"
    )]
    CutoverJournalRunMismatch {
        path: String,
        expected_run_id: String,
        actual_run_id: String,
        expected_customer_app_id: String,
        actual_customer_app_id: String,
    },
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
