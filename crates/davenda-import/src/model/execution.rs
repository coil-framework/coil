use super::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImportRecordStatus {
    Imported,
    Updated,
    SkippedUnchanged,
    StagedForReview,
    FailedValidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRecordReceipt {
    pub source_key: SourceRecordKey,
    pub target_id: Option<TargetRecordId>,
    pub batch_id: String,
    pub checksum: Option<String>,
    pub status: ImportRecordStatus,
}

impl ImportRecordReceipt {
    pub fn new(
        source_key: SourceRecordKey,
        batch_id: impl Into<String>,
        status: ImportRecordStatus,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            source_key,
            target_id: None,
            batch_id: require_non_empty("batch_id", batch_id.into())?,
            checksum: None,
            status,
        })
    }

    pub fn targeting(mut self, target_id: TargetRecordId) -> Self {
        self.target_id = Some(target_id);
        self
    }

    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Result<Self, ImportModelError> {
        self.checksum = Some(require_non_empty("checksum", checksum.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportRunSummary {
    receipts: Vec<ImportRecordReceipt>,
}

impl ImportRunSummary {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, receipt: ImportRecordReceipt) -> Result<(), ImportModelError> {
        if self
            .receipts
            .iter()
            .any(|existing| existing.source_key == receipt.source_key)
        {
            return Err(ImportModelError::DuplicateSourceReceipt {
                source_key: receipt.source_key.to_string(),
            });
        }

        self.receipts.push(receipt);
        Ok(())
    }

    pub fn status_counts(&self) -> BTreeMap<ImportRecordStatus, usize> {
        let mut counts = BTreeMap::new();
        for receipt in &self.receipts {
            *counts.entry(receipt.status).or_insert(0) += 1;
        }
        counts
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImporterExecutionStatus {
    Executed,
    SkippedCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImporterExecutionRecord {
    pub importer_id: String,
    pub phase: u16,
    pub resource_kind: String,
    pub description: String,
    pub batch_id: String,
    pub status: ImporterExecutionStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ImportJournal {
    run_id: String,
    customer_app_id: String,
    completed_importers: Vec<String>,
}

impl ImportJournal {
    pub(super) fn new(run_id: &ImportRunId, customer_app_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            customer_app_id: customer_app_id.to_string(),
            completed_importers: Vec::new(),
        }
    }

    pub(super) fn load(
        path: impl AsRef<Path>,
        run_id: &ImportRunId,
        customer_app_id: &str,
    ) -> Result<Self, ImportModelError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new(run_id, customer_app_id));
        }

        let input = fs::read_to_string(path).map_err(|error| ImportModelError::JournalRead {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let journal: Self =
            serde_json::from_str(&input).map_err(|error| ImportModelError::JournalParse {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
        if journal.run_id != run_id.as_str() || journal.customer_app_id != customer_app_id {
            return Err(ImportModelError::JournalRunMismatch {
                path: path.display().to_string(),
                expected_run_id: run_id.to_string(),
                actual_run_id: journal.run_id,
                expected_customer_app_id: customer_app_id.to_string(),
                actual_customer_app_id: journal.customer_app_id,
            });
        }
        Ok(journal)
    }

    pub(super) fn contains(&self, importer_id: &ImporterId) -> bool {
        self.completed_importers
            .iter()
            .any(|existing| existing == importer_id.as_str())
    }

    pub(super) fn mark_completed(&mut self, importer_id: &ImporterId) {
        if !self.contains(importer_id) {
            self.completed_importers.push(importer_id.to_string());
            self.completed_importers.sort();
        }
    }

    pub(super) fn save(&self, path: impl AsRef<Path>) -> Result<(), ImportModelError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| ImportModelError::JournalWrite {
                path: parent.display().to_string(),
                message: error.to_string(),
            })?;
        }
        let output =
            serde_json::to_string_pretty(self).map_err(|error| ImportModelError::JournalWrite {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
        fs::write(path, output).map_err(|error| ImportModelError::JournalWrite {
            path: path.display().to_string(),
            message: error.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportExecution {
    pub run_id: ImportRunId,
    pub customer_app_id: String,
    pub journal_path: String,
    pub importer_records: Vec<ImporterExecutionRecord>,
}

impl ImportExecution {
    pub fn command_report(&self) -> Result<CommandReport, ImportModelError> {
        let resumed = self
            .importer_records
            .iter()
            .any(|record| record.status == ImporterExecutionStatus::SkippedCompleted);
        let mut report = CommandReport::new(
            ["import", "run"],
            if resumed {
                format!(
                    "Resumed import run `{}` for `{}`",
                    self.run_id, self.customer_app_id
                )
            } else {
                format!(
                    "Executed import run `{}` for `{}`",
                    self.run_id, self.customer_app_id
                )
            },
        )?
        .with_columns([
            "phase",
            "importer",
            "resource_kind",
            "status",
            "batch_id",
            "description",
        ])?;

        for record in &self.importer_records {
            report.push_row(
                ReportRow::new()
                    .with_cell("phase", record.phase.to_string())?
                    .with_cell("importer", record.importer_id.clone())?
                    .with_cell("resource_kind", record.resource_kind.clone())?
                    .with_cell(
                        "status",
                        match record.status {
                            ImporterExecutionStatus::Executed => "executed".to_string(),
                            ImporterExecutionStatus::SkippedCompleted => {
                                "skipped_completed".to_string()
                            }
                        },
                    )?
                    .with_cell("batch_id", record.batch_id.clone())?
                    .with_cell("description", record.description.clone())?,
            );
        }

        report.push_diagnostic(DiagnosticRecord::new(
            DiagnosticSeverity::Info,
            "import.journal",
            format!("import journal persisted at `{}`", self.journal_path),
        )?);

        Ok(report)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackTrigger {
    pub id: RollbackTriggerId,
    pub description: String,
    pub fired: bool,
}

impl RollbackTrigger {
    pub fn new(
        id: RollbackTriggerId,
        description: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            id,
            description: require_non_empty("rollback_trigger_description", description.into())?,
            fired: false,
        })
    }

    pub fn fired(mut self) -> Self {
        self.fired = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CutoverCheck {
    pub id: String,
    pub description: String,
    pub required: bool,
    pub satisfied: bool,
}

impl CutoverCheck {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
        required: bool,
        satisfied: bool,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            id: validate_token("cutover_check_id", id.into())?,
            description: require_non_empty("cutover_check_description", description.into())?,
            required,
            satisfied,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CutoverPlan {
    pub checks: Vec<CutoverCheck>,
    pub rollback_triggers: Vec<RollbackTrigger>,
}

impl CutoverPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_check(mut self, check: CutoverCheck) -> Self {
        self.checks.push(check);
        self
    }

    pub fn with_trigger(mut self, trigger: RollbackTrigger) -> Self {
        self.rollback_triggers.push(trigger);
        self
    }

    pub fn is_ready(&self) -> bool {
        self.checks
            .iter()
            .filter(|check| check.required)
            .all(|check| check.satisfied)
            && self.rollback_triggers.iter().all(|trigger| !trigger.fired)
    }

    pub fn command_report(&self) -> Result<CommandReport, ImportModelError> {
        let mut report = CommandReport::new(
            ["release", "plan"],
            "Operational cutover and rollback readiness",
        )?
        .with_columns(["check", "required", "satisfied", "description"])?;
        report = report.with_status(if self.is_ready() {
            ReportStatus::Ok
        } else {
            ReportStatus::Unsafe
        });

        for check in &self.checks {
            report.push_row(
                ReportRow::new()
                    .with_cell("check", check.id.clone())?
                    .with_cell("required", check.required.to_string())?
                    .with_cell("satisfied", check.satisfied.to_string())?
                    .with_cell("description", check.description.clone())?,
            );
        }

        for trigger in &self.rollback_triggers {
            if trigger.fired {
                report.push_diagnostic(DiagnosticRecord::new(
                    DiagnosticSeverity::Error,
                    trigger.id.to_string(),
                    format!("rollback trigger fired: {}", trigger.description),
                )?);
            }
        }

        Ok(report)
    }
}
