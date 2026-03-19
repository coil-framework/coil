use super::*;

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
