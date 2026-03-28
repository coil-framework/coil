use serde::Serialize;
use std::collections::BTreeMap;

use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReportModelError {
    #[error("`{field}` cannot be empty")]
    EmptyField { field: &'static str },
    #[error("`{field}` contains an invalid token `{value}`")]
    InvalidToken { field: &'static str, value: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ReportStatus {
    Ok,
    Warning,
    Unsafe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticRecord {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
}

impl DiagnosticRecord {
    pub fn new(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Result<Self, ReportModelError> {
        Ok(Self {
            severity,
            code: validate_token("diagnostic_code", code.into())?,
            message: require_non_empty("diagnostic_message", message.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ReportRow {
    pub cells: BTreeMap<String, String>,
}

impl ReportRow {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_cell(
        mut self,
        column: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, ReportModelError> {
        let column = validate_token("report_column", column.into())?;
        let value = require_non_empty("report_value", value.into())?;
        self.cells.insert(column, value);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandReport {
    pub command: Vec<String>,
    pub status: ReportStatus,
    pub summary: String,
    pub columns: Vec<String>,
    pub rows: Vec<ReportRow>,
    pub diagnostics: Vec<DiagnosticRecord>,
}

impl CommandReport {
    pub fn new(
        command: impl IntoIterator<Item = impl Into<String>>,
        summary: impl Into<String>,
    ) -> Result<Self, ReportModelError> {
        let command = command
            .into_iter()
            .map(|segment| validate_token("command_segment", segment.into()))
            .collect::<Result<Vec<_>, _>>()?;
        if command.is_empty() {
            return Err(ReportModelError::EmptyField {
                field: "command_path",
            });
        }

        Ok(Self {
            command,
            status: ReportStatus::Ok,
            summary: require_non_empty("report_summary", summary.into())?,
            columns: Vec::new(),
            rows: Vec::new(),
            diagnostics: Vec::new(),
        })
    }

    pub fn with_status(mut self, status: ReportStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_columns(
        mut self,
        columns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, ReportModelError> {
        self.columns = columns
            .into_iter()
            .map(|column| validate_token("report_column", column.into()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    pub fn push_row(&mut self, row: ReportRow) {
        self.rows.push(row);
    }

    pub fn push_diagnostic(&mut self, diagnostic: DiagnosticRecord) {
        self.diagnostics.push(diagnostic);
    }
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, ReportModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ReportModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, ReportModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(ReportModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}
