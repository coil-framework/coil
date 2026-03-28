use super::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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

    pub fn receipts(&self) -> &[ImportRecordReceipt] {
        &self.receipts
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
    pub source_path: String,
    pub status: ImporterExecutionStatus,
    pub total_records: usize,
    pub imported_records: usize,
    pub updated_records: usize,
    pub skipped_records: usize,
    pub staged_records: usize,
    pub failed_records: usize,
    pub staged_path: Option<String>,
    pub exception_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct PersistedImportReceipt {
    target_id: Option<String>,
    checksum: Option<String>,
    status: Option<ImportRecordStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct ImporterJournalState {
    #[serde(default)]
    records: BTreeMap<String, PersistedImportReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ImportJournal {
    run_id: String,
    customer_app_id: String,
    #[serde(default)]
    completed_importers: Vec<String>,
    #[serde(default)]
    importer_states: BTreeMap<String, ImporterJournalState>,
}

impl ImportJournal {
    pub(super) fn new(run_id: &ImportRunId, customer_app_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            customer_app_id: customer_app_id.to_string(),
            completed_importers: Vec::new(),
            importer_states: BTreeMap::new(),
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

    fn previous_receipt(
        &self,
        importer_id: &ImporterId,
        source_key: &SourceRecordKey,
    ) -> Option<&PersistedImportReceipt> {
        self.importer_states
            .get(importer_id.as_str())
            .and_then(|state| state.records.get(source_key.as_str()))
    }

    fn resolved_target_for_kind(
        &self,
        importers: &[ImporterSpec],
        resource_kind: &str,
        source_key: &str,
    ) -> Option<String> {
        importers
            .iter()
            .filter(|importer| importer.resource_kind == resource_kind)
            .find_map(|importer| {
                self.importer_states
                    .get(importer.id.as_str())
                    .and_then(|state| state.records.get(source_key))
                    .and_then(|receipt| receipt.target_id.clone())
            })
    }

    fn record_receipt(&mut self, importer_id: &ImporterId, receipt: &ImportRecordReceipt) {
        let state = self
            .importer_states
            .entry(importer_id.to_string())
            .or_default();
        state.records.insert(
            receipt.source_key.to_string(),
            PersistedImportReceipt {
                target_id: receipt.target_id.as_ref().map(ToString::to_string),
                checksum: receipt.checksum.clone(),
                status: Some(receipt.status),
            },
        );
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoverExecutionState {
    Planned,
    FreezeConfirmed,
    Prepared,
    SwitchConfirmed,
    Observing,
    ObservationPassed,
    RollbackRequired,
    RolledBack,
    Failed,
}

impl CutoverExecutionState {
    fn label(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::FreezeConfirmed => "freeze_confirmed",
            Self::Prepared => "prepared",
            Self::SwitchConfirmed => "switch_confirmed",
            Self::Observing => "observing",
            Self::ObservationPassed => "observation_passed",
            Self::RollbackRequired => "rollback_required",
            Self::RolledBack => "rolled_back",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutoverStepStatus {
    Pending,
    Completed,
    Failed,
}

impl CutoverStepStatus {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutoverStepRecord {
    pub id: String,
    pub description: String,
    pub status: CutoverStepStatus,
    pub detail: Option<String>,
}

impl CutoverStepRecord {
    pub fn new(
        id: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            id: validate_token("cutover_step_id", id.into())?,
            description: require_non_empty("cutover_step_description", description.into())?,
            status: CutoverStepStatus::Pending,
            detail: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutoverDnsRecordChange {
    pub hostname: String,
    pub zone_id: String,
    pub record_id: String,
    pub record_type: String,
    pub previous_content: String,
    #[serde(default)]
    pub previous_proxied: Option<bool>,
    pub current_content: String,
    #[serde(default)]
    pub current_proxied: Option<bool>,
}

impl CutoverDnsRecordChange {
    pub fn new(
        hostname: impl Into<String>,
        zone_id: impl Into<String>,
        record_id: impl Into<String>,
        record_type: impl Into<String>,
        previous_content: impl Into<String>,
        current_content: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            hostname: require_non_empty("cutover_dns_hostname", hostname.into())?,
            zone_id: require_non_empty("cutover_dns_zone_id", zone_id.into())?,
            record_id: require_non_empty("cutover_dns_record_id", record_id.into())?,
            record_type: require_non_empty("cutover_dns_record_type", record_type.into())?,
            previous_content: require_non_empty(
                "cutover_dns_previous_content",
                previous_content.into(),
            )?,
            previous_proxied: None,
            current_content: require_non_empty(
                "cutover_dns_current_content",
                current_content.into(),
            )?,
            current_proxied: None,
        })
    }

    pub fn with_previous_proxied(mut self, proxied: Option<bool>) -> Self {
        self.previous_proxied = proxied;
        self
    }

    pub fn with_current_proxied(mut self, proxied: Option<bool>) -> Self {
        self.current_proxied = proxied;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutoverTrafficTargetChange {
    pub resource_kind: String,
    pub zone_id: String,
    pub resource_id: String,
    pub previous_target: String,
    pub current_target: String,
}

impl CutoverTrafficTargetChange {
    pub fn new(
        resource_kind: impl Into<String>,
        zone_id: impl Into<String>,
        resource_id: impl Into<String>,
        previous_target: impl Into<String>,
        current_target: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            resource_kind: validate_token("cutover_traffic_resource_kind", resource_kind.into())?,
            zone_id: require_non_empty("cutover_traffic_zone_id", zone_id.into())?,
            resource_id: require_non_empty("cutover_traffic_resource_id", resource_id.into())?,
            previous_target: require_non_empty(
                "cutover_traffic_previous_target",
                previous_target.into(),
            )?,
            current_target: require_non_empty(
                "cutover_traffic_current_target",
                current_target.into(),
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutoverSwitchExecution {
    pub method: String,
    #[serde(default)]
    pub dns_records: Vec<CutoverDnsRecordChange>,
    #[serde(default)]
    pub traffic_targets: Vec<CutoverTrafficTargetChange>,
}

impl CutoverSwitchExecution {
    pub fn new(method: impl Into<String>) -> Result<Self, ImportModelError> {
        Ok(Self {
            method: validate_token("cutover_switch_method", method.into())?,
            dns_records: Vec::new(),
            traffic_targets: Vec::new(),
        })
    }

    pub fn with_dns_record(mut self, record: CutoverDnsRecordChange) -> Self {
        self.dns_records.push(record);
        self
    }

    pub fn with_traffic_target(mut self, target: CutoverTrafficTargetChange) -> Self {
        self.traffic_targets.push(target);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CutoverExecutionJournal {
    run_id: String,
    customer_app_id: String,
    pub state: CutoverExecutionState,
    #[serde(default)]
    pub switch_confirmed_at_unix_seconds: Option<u64>,
    #[serde(default)]
    pub observation_started_at_unix_seconds: Option<u64>,
    #[serde(default)]
    pub rollback_confirmed_at_unix_seconds: Option<u64>,
    #[serde(default)]
    pub observation_base_url: Option<String>,
    #[serde(default)]
    pub last_probe_at_unix_seconds: Option<u64>,
    #[serde(default)]
    pub last_probe_failures: Vec<String>,
    #[serde(default)]
    pub rollback_reason: Option<String>,
    #[serde(default)]
    pub switch_execution: Option<CutoverSwitchExecution>,
    #[serde(default)]
    pub steps: Vec<CutoverStepRecord>,
}

impl CutoverExecutionJournal {
    pub fn new(run_id: &ImportRunId, customer_app_id: &str, steps: Vec<CutoverStepRecord>) -> Self {
        Self {
            run_id: run_id.to_string(),
            customer_app_id: customer_app_id.to_string(),
            state: CutoverExecutionState::Planned,
            switch_confirmed_at_unix_seconds: None,
            observation_started_at_unix_seconds: None,
            rollback_confirmed_at_unix_seconds: None,
            observation_base_url: None,
            last_probe_at_unix_seconds: None,
            last_probe_failures: Vec::new(),
            rollback_reason: None,
            switch_execution: None,
            steps,
        }
    }

    pub fn load(
        path: impl AsRef<Path>,
        run_id: &ImportRunId,
        customer_app_id: &str,
        expected_steps: Vec<CutoverStepRecord>,
    ) -> Result<Self, ImportModelError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::new(run_id, customer_app_id, expected_steps));
        }

        let input =
            fs::read_to_string(path).map_err(|error| ImportModelError::CutoverJournalRead {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
        let mut journal: Self = serde_json::from_str(&input).map_err(|error| {
            ImportModelError::CutoverJournalParse {
                path: path.display().to_string(),
                message: error.to_string(),
            }
        })?;
        if journal.run_id != run_id.as_str() || journal.customer_app_id != customer_app_id {
            return Err(ImportModelError::CutoverJournalRunMismatch {
                path: path.display().to_string(),
                expected_run_id: run_id.to_string(),
                actual_run_id: journal.run_id,
                expected_customer_app_id: customer_app_id.to_string(),
                actual_customer_app_id: journal.customer_app_id,
            });
        }

        for expected in expected_steps {
            if !journal
                .steps
                .iter()
                .any(|existing| existing.id == expected.id)
            {
                journal.steps.push(expected);
            }
        }

        Ok(journal)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ImportModelError> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| ImportModelError::CutoverJournalWrite {
                path: parent.display().to_string(),
                message: error.to_string(),
            })?;
        }
        let output = serde_json::to_string_pretty(self).map_err(|error| {
            ImportModelError::CutoverJournalWrite {
                path: path.display().to_string(),
                message: error.to_string(),
            }
        })?;
        fs::write(path, output).map_err(|error| ImportModelError::CutoverJournalWrite {
            path: path.display().to_string(),
            message: error.to_string(),
        })
    }

    pub fn confirm_freeze(&mut self) {
        if self.state == CutoverExecutionState::Planned {
            self.state = CutoverExecutionState::FreezeConfirmed;
        }
    }

    pub fn step_completed(&self, step_id: &str) -> bool {
        self.steps
            .iter()
            .find(|step| step.id == step_id)
            .is_some_and(|step| step.status == CutoverStepStatus::Completed)
    }

    pub fn mark_step_completed(
        &mut self,
        step_id: &str,
        detail: impl Into<String>,
    ) -> Result<(), ImportModelError> {
        let detail = require_non_empty("cutover_step_detail", detail.into())?;
        if let Some(step) = self.steps.iter_mut().find(|step| step.id == step_id) {
            step.status = CutoverStepStatus::Completed;
            step.detail = Some(detail);
        }
        Ok(())
    }

    pub fn mark_step_failed(
        &mut self,
        step_id: &str,
        detail: impl Into<String>,
    ) -> Result<(), ImportModelError> {
        let detail = require_non_empty("cutover_step_detail", detail.into())?;
        self.state = CutoverExecutionState::Failed;
        if let Some(step) = self.steps.iter_mut().find(|step| step.id == step_id) {
            step.status = CutoverStepStatus::Failed;
            step.detail = Some(detail);
        }
        Ok(())
    }

    pub fn mark_prepared(&mut self) {
        self.state = CutoverExecutionState::Prepared;
    }

    pub fn record_switch_execution(&mut self, execution: CutoverSwitchExecution) {
        self.switch_execution = Some(execution);
    }

    pub fn confirm_switch(&mut self, base_url: impl Into<String>, at_unix_seconds: u64) {
        self.switch_confirmed_at_unix_seconds = Some(at_unix_seconds);
        self.observation_base_url = Some(base_url.into());
        if self.state == CutoverExecutionState::Prepared {
            self.state = CutoverExecutionState::SwitchConfirmed;
        }
    }

    pub fn begin_observation(&mut self, base_url: impl Into<String>, at_unix_seconds: u64) {
        self.confirm_switch(base_url.into(), at_unix_seconds);
        if self.observation_started_at_unix_seconds.is_none() {
            self.observation_started_at_unix_seconds = Some(at_unix_seconds);
        }
        self.last_probe_at_unix_seconds = Some(at_unix_seconds);
        self.last_probe_failures.clear();
        self.state = CutoverExecutionState::Observing;
    }

    pub fn mark_observation_passed(&mut self, at_unix_seconds: u64) {
        self.last_probe_at_unix_seconds = Some(at_unix_seconds);
        self.last_probe_failures.clear();
        self.state = CutoverExecutionState::ObservationPassed;
    }

    pub fn mark_rollback_required(
        &mut self,
        base_url: impl Into<String>,
        at_unix_seconds: u64,
        failures: Vec<String>,
    ) {
        self.confirm_switch(base_url.into(), at_unix_seconds);
        if self.observation_started_at_unix_seconds.is_none() {
            self.observation_started_at_unix_seconds = Some(at_unix_seconds);
        }
        self.last_probe_at_unix_seconds = Some(at_unix_seconds);
        self.last_probe_failures = failures;
        self.state = CutoverExecutionState::RollbackRequired;
    }

    pub fn mark_rolled_back(
        &mut self,
        base_url: impl Into<String>,
        at_unix_seconds: u64,
        reason: impl Into<String>,
    ) -> Result<(), ImportModelError> {
        self.confirm_switch(base_url.into(), at_unix_seconds);
        self.rollback_confirmed_at_unix_seconds = Some(at_unix_seconds);
        self.rollback_reason = Some(require_non_empty("rollback_reason", reason.into())?);
        self.last_probe_at_unix_seconds = Some(at_unix_seconds);
        self.state = CutoverExecutionState::RolledBack;
        Ok(())
    }

    pub fn command_report(&self) -> Result<CommandReport, ImportModelError> {
        let mut report = CommandReport::new(
            ["import", "cutover"],
            format!(
                "Cutover execution for import run `{}` is currently `{}`",
                self.run_id,
                self.state.label()
            ),
        )?
        .with_columns(["step", "status", "description", "detail"])?;
        report = report.with_status(match self.state {
            CutoverExecutionState::Prepared | CutoverExecutionState::ObservationPassed => {
                ReportStatus::Ok
            }
            CutoverExecutionState::RollbackRequired | CutoverExecutionState::Failed => {
                ReportStatus::Unsafe
            }
            CutoverExecutionState::RolledBack => ReportStatus::Warning,
            CutoverExecutionState::Planned
            | CutoverExecutionState::FreezeConfirmed
            | CutoverExecutionState::SwitchConfirmed
            | CutoverExecutionState::Observing => ReportStatus::Warning,
        });

        for step in &self.steps {
            report.push_row(
                ReportRow::new()
                    .with_cell("step", step.id.clone())?
                    .with_cell("status", step.status.label())?
                    .with_cell("description", step.description.clone())?
                    .with_cell(
                        "detail",
                        step.detail.clone().unwrap_or_else(|| "pending".to_string()),
                    )?,
            );
        }

        Ok(report)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportExecution {
    pub run_id: ImportRunId,
    pub customer_app_id: String,
    pub journal_path: String,
    pub run_root: String,
    pub importer_records: Vec<ImporterExecutionRecord>,
    pub summary: ImportRunSummary,
}

impl ImportExecution {
    pub fn command_report(&self) -> Result<CommandReport, ImportModelError> {
        let resumed = self
            .importer_records
            .iter()
            .all(|record| record.status == ImporterExecutionStatus::SkippedCompleted);
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
            "source",
            "total",
            "imported",
            "updated",
            "skipped",
            "staged",
            "failed",
        ])?;

        let mut overall_status = ReportStatus::Ok;
        for record in &self.importer_records {
            if record.failed_records > 0 {
                overall_status = ReportStatus::Unsafe;
            } else if record.staged_records > 0 && overall_status != ReportStatus::Unsafe {
                overall_status = ReportStatus::Warning;
            }

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
                    .with_cell("source", record.source_path.clone())?
                    .with_cell("total", record.total_records.to_string())?
                    .with_cell("imported", record.imported_records.to_string())?
                    .with_cell("updated", record.updated_records.to_string())?
                    .with_cell("skipped", record.skipped_records.to_string())?
                    .with_cell("staged", record.staged_records.to_string())?
                    .with_cell("failed", record.failed_records.to_string())?,
            );

            if let Some(staged_path) = &record.staged_path {
                report.push_diagnostic(DiagnosticRecord::new(
                    DiagnosticSeverity::Info,
                    format!("import.{}.staged", record.importer_id),
                    format!("validated records staged at `{staged_path}`"),
                )?);
            }
            if let Some(exception_path) = &record.exception_path {
                let severity = if record.failed_records > 0 {
                    DiagnosticSeverity::Warning
                } else {
                    DiagnosticSeverity::Info
                };
                report.push_diagnostic(DiagnosticRecord::new(
                    severity,
                    format!("import.{}.exceptions", record.importer_id),
                    format!("staged exceptions persisted at `{exception_path}`"),
                )?);
            }
        }

        report = report.with_status(overall_status);
        report.push_diagnostic(DiagnosticRecord::new(
            DiagnosticSeverity::Info,
            "import.journal",
            format!("import journal persisted at `{}`", self.journal_path),
        )?);
        report.push_diagnostic(DiagnosticRecord::new(
            DiagnosticSeverity::Info,
            "import.run_root",
            format!("import artifacts persisted at `{}`", self.run_root),
        )?);

        Ok(report)
    }
}

pub(crate) fn execute_import_plan_with_handler<F>(
    plan: &ImportPlan,
    manifest_root: &Path,
    journal_path: &Path,
    handler: F,
) -> Result<ImportExecution, ImportModelError>
where
    F: FnMut(
        &ImporterSpec,
        &ImportRecordReceipt,
        &Path,
        &mut Value,
    ) -> Result<(), ImportModelError>,
{
    execute_import_plan_internal(plan, manifest_root, journal_path, Some(handler))
}

fn execute_import_plan_internal<F>(
    plan: &ImportPlan,
    manifest_root: &Path,
    journal_path: &Path,
    mut handler: Option<F>,
) -> Result<ImportExecution, ImportModelError>
where
    F: FnMut(
        &ImporterSpec,
        &ImportRecordReceipt,
        &Path,
        &mut Value,
    ) -> Result<(), ImportModelError>,
{
    let mut journal = ImportJournal::load(journal_path, &plan.run_id, &plan.customer_app_id)?;
    let run_root = import_run_root(journal_path);
    fs::create_dir_all(run_root.join("staged")).map_err(|error| {
        ImportModelError::ArtifactWrite {
            path: run_root.join("staged").display().to_string(),
            message: error.to_string(),
        }
    })?;
    fs::create_dir_all(run_root.join("exceptions")).map_err(|error| {
        ImportModelError::ArtifactWrite {
            path: run_root.join("exceptions").display().to_string(),
            message: error.to_string(),
        }
    })?;

    let mut importer_records = Vec::with_capacity(plan.ordered_importers.len());
    let mut summary = ImportRunSummary::new();

    for importer in &plan.ordered_importers {
        let source_rel = importer.source_path.as_ref().ok_or_else(|| {
            ImportModelError::MissingImporterSourcePath {
                importer_id: importer.id.to_string(),
            }
        })?;
        let source_path = resolve_source_path(manifest_root, source_rel);
        let raw_records = load_source_records(importer, &source_path)?;
        let staged_path = run_root
            .join("staged")
            .join(format!("{}.json", importer.id));
        let exception_path = run_root
            .join("exceptions")
            .join(format!("{}.json", importer.id));

        let mut staged_records = Vec::new();
        let mut exception_records = Vec::new();
        let mut imported_records = 0usize;
        let mut updated_records = 0usize;
        let mut skipped_records = 0usize;
        let mut staged_records_count = 0usize;
        let mut failed_records = 0usize;

        for (index, raw_record) in raw_records.into_iter().enumerate() {
            let record_ref = record_identifier(&raw_record, index + 1);
            match process_record(plan, importer, &journal, &raw_record) {
                Ok((receipt, Some(mut staged_record))) => {
                    if let Some(handler) = handler.as_mut() {
                        handler(importer, &receipt, manifest_root, &mut staged_record).map_err(
                            |error| ImportModelError::ExecutionHook {
                                importer_id: importer.id.to_string(),
                                record: receipt.source_key.to_string(),
                                message: error.to_string(),
                            },
                        )?;
                    }
                    update_counts(
                        receipt.status,
                        &mut imported_records,
                        &mut updated_records,
                        &mut skipped_records,
                        &mut staged_records_count,
                        &mut failed_records,
                    );
                    journal.record_receipt(&importer.id, &receipt);
                    summary.record(receipt)?;
                    staged_records.push(staged_record);
                    persist_artifact(&staged_path, &staged_records)?;
                    journal.save(journal_path)?;
                }
                Ok((receipt, None)) => {
                    update_counts(
                        receipt.status,
                        &mut imported_records,
                        &mut updated_records,
                        &mut skipped_records,
                        &mut staged_records_count,
                        &mut failed_records,
                    );
                    journal.record_receipt(&importer.id, &receipt);
                    summary.record(receipt)?;
                    journal.save(journal_path)?;
                }
                Err(error) => {
                    if plan.validation_mode == ValidationMode::Strict {
                        return Err(ImportModelError::InvalidSourceRecord {
                            importer_id: importer.id.to_string(),
                            record: record_ref,
                            message: error,
                        });
                    }

                    let source_key = SourceRecordKey::new(format!("{}:{record_ref}", importer.id))
                        .map_err(|source_error| ImportModelError::InvalidSourceRecord {
                            importer_id: importer.id.to_string(),
                            record: record_ref.clone(),
                            message: source_error.to_string(),
                        })?;
                    let receipt = ImportRecordReceipt::new(
                        source_key,
                        format!("{}:{}", plan.run_id, importer.id),
                        ImportRecordStatus::FailedValidation,
                    )?;
                    update_counts(
                        receipt.status,
                        &mut imported_records,
                        &mut updated_records,
                        &mut skipped_records,
                        &mut staged_records_count,
                        &mut failed_records,
                    );
                    journal.record_receipt(&importer.id, &receipt);
                    summary.record(receipt)?;
                    exception_records.push(json!({
                        "record": record_ref,
                        "error": error,
                        "raw": raw_record,
                    }));
                    persist_artifact(&exception_path, &exception_records)?;
                    journal.save(journal_path)?;
                }
            }
        }

        importer_records.push(ImporterExecutionRecord {
            importer_id: importer.id.to_string(),
            phase: importer.phase,
            resource_kind: importer.resource_kind.clone(),
            description: importer.description.clone(),
            batch_id: format!("{}:{}", plan.run_id, importer.id),
            source_path: source_path.display().to_string(),
            status: if skipped_records > 0
                && skipped_records
                    == imported_records
                        + updated_records
                        + skipped_records
                        + staged_records_count
                        + failed_records
            {
                ImporterExecutionStatus::SkippedCompleted
            } else {
                ImporterExecutionStatus::Executed
            },
            total_records: imported_records
                + updated_records
                + skipped_records
                + staged_records_count
                + failed_records,
            imported_records,
            updated_records,
            skipped_records,
            staged_records: staged_records_count,
            failed_records,
            staged_path: staged_path
                .exists()
                .then(|| staged_path.display().to_string()),
            exception_path: exception_path
                .exists()
                .then(|| exception_path.display().to_string()),
        });
    }

    Ok(ImportExecution {
        run_id: plan.run_id.clone(),
        customer_app_id: plan.customer_app_id.clone(),
        journal_path: journal_path.display().to_string(),
        run_root: run_root.display().to_string(),
        importer_records,
        summary,
    })
}

fn import_run_root(journal_path: &Path) -> PathBuf {
    if journal_path.extension().is_some() {
        journal_path.with_extension("")
    } else {
        journal_path.to_path_buf()
    }
}

fn resolve_source_path(manifest_root: &Path, source_path: &str) -> PathBuf {
    let source = PathBuf::from(source_path);
    if source.is_absolute() {
        source
    } else {
        manifest_root.join(source)
    }
}

fn load_source_records(
    importer: &ImporterSpec,
    source_path: &Path,
) -> Result<Vec<Value>, ImportModelError> {
    match importer.source_format {
        ImportSourceFormat::Json => {}
    }
    let input = fs::read_to_string(source_path).map_err(|error| ImportModelError::SourceRead {
        importer_id: importer.id.to_string(),
        path: source_path.display().to_string(),
        message: error.to_string(),
    })?;
    let value: Value =
        serde_json::from_str(&input).map_err(|error| ImportModelError::SourceParse {
            importer_id: importer.id.to_string(),
            path: source_path.display().to_string(),
            message: error.to_string(),
        })?;

    match value {
        Value::Array(records) => Ok(records),
        Value::Object(mut object) => object
            .remove("records")
            .and_then(|records| records.as_array().cloned())
            .ok_or_else(|| ImportModelError::SourceShape {
                importer_id: importer.id.to_string(),
                path: source_path.display().to_string(),
            }),
        _ => Err(ImportModelError::SourceShape {
            importer_id: importer.id.to_string(),
            path: source_path.display().to_string(),
        }),
    }
}

fn process_record(
    plan: &ImportPlan,
    importer: &ImporterSpec,
    journal: &ImportJournal,
    raw_record: &Value,
) -> Result<(ImportRecordReceipt, Option<Value>), String> {
    let source_key = SourceRecordKey::new(required_string(raw_record, "source_key")?)
        .map_err(|error| error.to_string())?;
    let checksum =
        optional_string(raw_record, "checksum")?.unwrap_or_else(|| canonical_checksum(raw_record));
    if journal
        .previous_receipt(&importer.id, &source_key)
        .and_then(|receipt| receipt.checksum.as_deref())
        == Some(checksum.as_str())
    {
        let mut receipt = ImportRecordReceipt::new(
            source_key.clone(),
            format!("{}:{}", plan.run_id, importer.id),
            ImportRecordStatus::SkippedUnchanged,
        )
        .map_err(|error| error.to_string())?;
        if let Some(target_id) = journal
            .previous_receipt(&importer.id, &source_key)
            .and_then(|receipt| receipt.target_id.as_deref())
        {
            receipt = receipt.targeting(
                TargetRecordId::new(target_id.to_string()).map_err(|error| error.to_string())?,
            );
        }
        receipt = receipt
            .with_checksum(checksum)
            .map_err(|error| error.to_string())?;
        return Ok((receipt, None));
    }

    let transformed =
        transform_record(plan, importer, journal, raw_record, &source_key, &checksum)?;
    let status = match plan.publication_mode {
        PublicationMode::PublishValidated => {
            if journal
                .previous_receipt(&importer.id, &source_key)
                .is_some()
            {
                ImportRecordStatus::Updated
            } else {
                ImportRecordStatus::Imported
            }
        }
        PublicationMode::StageValidated | PublicationMode::ValidateOnly => {
            ImportRecordStatus::StagedForReview
        }
    };

    let mut receipt = ImportRecordReceipt::new(
        source_key.clone(),
        format!("{}:{}", plan.run_id, importer.id),
        status,
    )
    .map_err(|error| error.to_string())?
    .targeting(
        TargetRecordId::new(transformed.target_id.clone()).map_err(|error| error.to_string())?,
    );
    receipt = receipt
        .with_checksum(checksum.clone())
        .map_err(|error| error.to_string())?;

    Ok((
        receipt,
        Some(json!({
            "source_system": plan.source_system.as_str(),
            "importer_id": importer.id.as_str(),
            "resource_kind": importer.resource_kind,
            "source_key": source_key.as_str(),
            "target_id": transformed.target_id,
            "checksum": checksum,
            "publication_mode": publication_mode_label(plan.publication_mode),
            "validation_mode": validation_mode_label(plan.validation_mode),
            "mapping": importer.mapping,
            "normalized": transformed.normalized,
        })),
    ))
}

struct TransformedRecord {
    target_id: String,
    normalized: Value,
}

fn transform_record(
    plan: &ImportPlan,
    importer: &ImporterSpec,
    journal: &ImportJournal,
    raw_record: &Value,
    source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    match importer.resource_kind.as_str() {
        "page" => transform_page(plan, importer, journal, raw_record, source_key, checksum),
        "asset" => transform_asset(plan, importer, raw_record, source_key, checksum),
        "user" => transform_user(plan, importer, raw_record, source_key, checksum),
        "event" => transform_event(plan, importer, journal, raw_record, source_key, checksum),
        "membership_tier" => {
            transform_membership_tier(plan, importer, raw_record, source_key, checksum)
        }
        "subscription" => {
            transform_subscription(plan, importer, journal, raw_record, source_key, checksum)
        }
        other => Err(ImportModelError::UnsupportedResourceKind {
            importer_id: importer.id.to_string(),
            resource_kind: other.to_string(),
        }
        .to_string()),
    }
}

fn transform_page(
    plan: &ImportPlan,
    importer: &ImporterSpec,
    journal: &ImportJournal,
    raw_record: &Value,
    _source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    let slug = validate_token("page_slug", required_string(raw_record, "slug")?)
        .map_err(|error| error.to_string())?;
    let title = required_string(raw_record, "title")?;
    let body_html = required_string(raw_record, "body_html")?;
    let template = importer
        .mapping
        .get("template")
        .cloned()
        .or_else(|| optional_string(raw_record, "template").ok().flatten())
        .unwrap_or_else(|| "pages/home".to_string());
    let page_type = importer
        .mapping
        .get("page_type")
        .cloned()
        .or_else(|| optional_string(raw_record, "page_type").ok().flatten())
        .unwrap_or_else(|| "landing_page".to_string());
    let locale = optional_string(raw_record, "locale")?.or_else(|| plan.locale.clone());
    let canonical_path = optional_string(raw_record, "canonical_path")?;
    if canonical_path
        .as_ref()
        .is_some_and(|path| !path.starts_with('/'))
    {
        return Err("canonical_path must start with `/`".to_string());
    }

    let media_references = optional_string_array(raw_record, "media_references")?
        .into_iter()
        .map(|reference| {
            journal
                .resolved_target_for_kind(&plan.ordered_importers, "asset", &reference)
                .ok_or_else(|| {
                    format!("media reference `{reference}` does not resolve to an imported asset")
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let target_id =
        optional_string(raw_record, "target_id")?.unwrap_or_else(|| format!("page:{slug}"));

    Ok(TransformedRecord {
        target_id,
        normalized: json!({
            "kind": "page",
            "page_type": page_type,
            "title": title,
            "slug": slug,
            "template": template,
            "body_html": body_html,
            "locale": locale,
            "site": plan.site,
            "seo": {
                "title": optional_string(raw_record, "seo_title")?,
                "description": optional_string(raw_record, "seo_description")?,
                "canonical_path": canonical_path,
            },
            "media_references": media_references,
            "publication_state": publication_state_label(plan.publication_mode),
            "fingerprint": checksum,
        }),
    })
}

fn transform_asset(
    plan: &ImportPlan,
    _importer: &ImporterSpec,
    raw_record: &Value,
    _source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    let slug = validate_token("asset_slug", required_string(raw_record, "slug")?)
        .map_err(|error| error.to_string())?;
    let title = required_string(raw_record, "title")?;
    let source_url = optional_string(raw_record, "source_url")?;
    let source_object_key = optional_string(raw_record, "source_object_key")?;
    let source_file = optional_string(raw_record, "source_file")?;
    if source_url.is_none() && source_object_key.is_none() && source_file.is_none() {
        return Err(
            "asset record must define `source_url`, `source_object_key`, or `source_file`"
                .to_string(),
        );
    }
    let target_id =
        optional_string(raw_record, "target_id")?.unwrap_or_else(|| format!("asset:{slug}"));
    let folder = optional_string(raw_record, "folder")?;
    let storage_class = optional_string(raw_record, "storage_class")?
        .unwrap_or_else(|| asset_storage_default_label(plan.asset_storage_default).to_string());

    Ok(TransformedRecord {
        target_id,
        normalized: json!({
            "kind": "asset",
            "title": title,
            "slug": slug,
            "folder": folder,
            "content_type": required_string(raw_record, "content_type")?,
            "source_url": source_url,
            "source_object_key": source_object_key,
            "source_file": source_file,
            "source_etag": optional_string(raw_record, "source_etag")?,
            "alt_text": optional_string(raw_record, "alt_text")?,
            "caption": optional_string(raw_record, "caption")?,
            "copyright": optional_string(raw_record, "copyright")?,
            "storage_class": storage_class,
            "publication_state": publication_state_label(plan.publication_mode),
            "fingerprint": checksum,
            "logical_path": match folder.as_deref() {
                Some(folder) => format!("{}/{folder}/{slug}", plan.customer_app_id),
                None => format!("{}/{}", plan.customer_app_id, slug),
            }
        }),
    })
}

fn transform_user(
    _plan: &ImportPlan,
    _importer: &ImporterSpec,
    raw_record: &Value,
    _source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    let email = optional_string(raw_record, "email")?;
    let username = optional_string(raw_record, "username")?;
    let principal_id = optional_string(raw_record, "principal_id")?
        .or_else(|| username.clone())
        .ok_or_else(|| {
            "user record must define `principal_id` or `username` for live auth import".to_string()
        })?;
    let legacy_roles = optional_string_array(raw_record, "legacy_roles")?;
    let target_id =
        optional_string(raw_record, "target_id")?.unwrap_or_else(|| principal_id.clone());

    Ok(TransformedRecord {
        target_id,
        normalized: json!({
            "kind": "user",
            "principal_id": principal_id,
            "email": email,
            "username": username,
            "display_name": optional_string(raw_record, "display_name")?,
            "legacy_roles": legacy_roles,
            "fingerprint": checksum,
        }),
    })
}

fn transform_event(
    plan: &ImportPlan,
    _importer: &ImporterSpec,
    journal: &ImportJournal,
    raw_record: &Value,
    _source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    let slug = validate_token("event_slug", required_string(raw_record, "slug")?)
        .map_err(|error| error.to_string())?;
    let target_id =
        optional_string(raw_record, "target_id")?.unwrap_or_else(|| format!("event:{slug}"));
    let hero_asset = optional_string(raw_record, "hero_asset_source_key")?.map(|reference| {
        journal
            .resolved_target_for_kind(&plan.ordered_importers, "asset", &reference)
            .unwrap_or(reference)
    });

    Ok(TransformedRecord {
        target_id,
        normalized: json!({
            "kind": "event",
            "title": required_string(raw_record, "title")?,
            "slug": slug,
            "starts_at": required_string(raw_record, "starts_at")?,
            "ends_at": optional_string(raw_record, "ends_at")?,
            "summary": optional_string(raw_record, "summary")?,
            "hero_asset": hero_asset,
            "fingerprint": checksum,
        }),
    })
}

fn transform_membership_tier(
    _plan: &ImportPlan,
    _importer: &ImporterSpec,
    raw_record: &Value,
    _source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    let tier_id = optional_string(raw_record, "tier_id")?
        .or_else(|| optional_string(raw_record, "slug").ok().flatten())
        .ok_or_else(|| "membership tier record must define `tier_id` or `slug`".to_string())?;
    let tier_id =
        validate_token("membership_tier_id", tier_id).map_err(|error| error.to_string())?;
    let target_id = optional_string(raw_record, "target_id")?.unwrap_or_else(|| tier_id.clone());
    let interval = validate_membership_interval(
        optional_string(raw_record, "interval")?.unwrap_or_else(|| "monthly".to_string()),
    )?;
    let visibility = validate_membership_visibility(
        optional_string(raw_record, "visibility")?.unwrap_or_else(|| "public".to_string()),
    )?;

    Ok(TransformedRecord {
        target_id,
        normalized: json!({
            "kind": "membership_tier",
            "title": required_string(raw_record, "title")?,
            "tier_id": tier_id,
            "entitlement_key": required_string(raw_record, "entitlement_key")?,
            "rank": optional_u64(raw_record, "rank")?.unwrap_or_default(),
            "interval": interval,
            "grace_period_days": optional_u64(raw_record, "grace_period_days")?.unwrap_or_default(),
            "visibility": visibility,
            "status": optional_string(raw_record, "status")?.unwrap_or_else(|| "active".to_string()),
            "fingerprint": checksum,
        }),
    })
}

fn transform_subscription(
    plan: &ImportPlan,
    _importer: &ImporterSpec,
    journal: &ImportJournal,
    raw_record: &Value,
    _source_key: &SourceRecordKey,
    checksum: &str,
) -> Result<TransformedRecord, String> {
    let subscription_id = optional_string(raw_record, "subscription_id")?
        .or_else(|| optional_string(raw_record, "slug").ok().flatten())
        .ok_or_else(|| "subscription record must define `subscription_id` or `slug`".to_string())?;
    let subscription_id =
        validate_token("subscription_id", subscription_id).map_err(|error| error.to_string())?;
    let target_id =
        optional_string(raw_record, "target_id")?.unwrap_or_else(|| subscription_id.clone());
    let tier_id = match optional_string(raw_record, "tier_source_key")? {
        Some(reference) => journal
            .resolved_target_for_kind(&plan.ordered_importers, "membership_tier", &reference)
            .ok_or_else(|| {
                format!(
                    "membership tier reference `{reference}` does not resolve to an imported tier"
                )
            })?,
        None => required_string(raw_record, "tier_id")?,
    };
    let principal_id = match optional_string(raw_record, "user_source_key")? {
        Some(reference) => journal
            .resolved_target_for_kind(&plan.ordered_importers, "user", &reference)
            .ok_or_else(|| {
                format!("user reference `{reference}` does not resolve to an imported user")
            })?,
        None => required_string(raw_record, "principal_id")?,
    };
    let status = required_string(raw_record, "status")?;
    if !matches!(status.as_str(), "active" | "in_grace_period") {
        return Err(format!(
            "subscription status `{status}` is not supported by the current live import path"
        ));
    }
    let entitlement_key = required_string(raw_record, "entitlement_key")?;
    let renews_at = required_u64(raw_record, "renews_at")?;
    let grace_period_ends_at = match status.as_str() {
        "in_grace_period" => Some(required_u64(raw_record, "grace_period_ends_at")?),
        _ => optional_u64(raw_record, "grace_period_ends_at")?,
    };
    let active = matches!(status.as_str(), "active" | "in_grace_period");

    Ok(TransformedRecord {
        target_id,
        normalized: json!({
            "kind": "subscription",
            "subscription_id": subscription_id,
            "tier_id": tier_id,
            "principal_id": principal_id,
            "email": optional_string(raw_record, "email")?,
            "username": optional_string(raw_record, "username")?,
            "display_name": optional_string(raw_record, "display_name")?,
            "status": status,
            "entitlement_key": entitlement_key,
            "entitlement_id": format!("entitlement:{subscription_id}"),
            "active": active,
            "renews_at": renews_at,
            "grace_period_ends_at": grace_period_ends_at,
            "fingerprint": checksum,
        }),
    })
}

fn required_string(record: &Value, field: &'static str) -> Result<String, String> {
    match record.get(field).and_then(Value::as_str) {
        Some(value) => {
            require_non_empty(field, value.to_string()).map_err(|error| error.to_string())
        }
        None => Err(format!("missing required `{field}`")),
    }
}

fn optional_string(record: &Value, field: &'static str) -> Result<Option<String>, String> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => require_non_empty(field, value.clone())
            .map(Some)
            .map_err(|error| error.to_string()),
        Some(_) => Err(format!("`{field}` must be a string")),
    }
}

fn optional_string_array(record: &Value, field: &'static str) -> Result<Vec<String>, String> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| match value.as_str() {
                Some(item) => {
                    require_non_empty(field, item.to_string()).map_err(|error| error.to_string())
                }
                None => Err(format!("`{field}` entries must be strings")),
            })
            .collect(),
        Some(_) => Err(format!("`{field}` must be an array of strings")),
    }
}

fn optional_u64(record: &Value, field: &'static str) -> Result<Option<u64>, String> {
    match record.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| format!("`{field}` must be an unsigned integer")),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => value
            .parse::<u64>()
            .map(Some)
            .map_err(|_| format!("`{field}` must be an unsigned integer")),
        Some(_) => Err(format!("`{field}` must be an unsigned integer")),
    }
}

fn required_u64(record: &Value, field: &'static str) -> Result<u64, String> {
    optional_u64(record, field)?.ok_or_else(|| format!("missing required `{field}`"))
}

fn validate_membership_interval(value: String) -> Result<String, String> {
    match value.as_str() {
        "monthly" | "quarterly" | "annual" => Ok(value),
        _ if value
            .strip_prefix("custom_days:")
            .is_some_and(|days| days.parse::<u16>().is_ok()) =>
        {
            Ok(value)
        }
        _ => Err(format!(
            "membership tier interval `{value}` must be `monthly`, `quarterly`, `annual`, or `custom_days:<days>`"
        )),
    }
}

fn validate_membership_visibility(value: String) -> Result<String, String> {
    match value.as_str() {
        "public" | "invite_only" | "staff_managed" => Ok(value),
        _ => Err(format!(
            "membership tier visibility `{value}` must be `public`, `invite_only`, or `staff_managed`"
        )),
    }
}

fn canonical_checksum(raw_record: &Value) -> String {
    serde_json::to_string(raw_record).unwrap_or_else(|_| "<invalid-json>".to_string())
}

fn record_identifier(raw_record: &Value, index: usize) -> String {
    raw_record
        .get("source_key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("record-{index}"))
}

fn persist_artifact(path: &Path, records: &[Value]) -> Result<(), ImportModelError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| ImportModelError::ArtifactWrite {
            path: parent.display().to_string(),
            message: error.to_string(),
        })?;
    }
    let output =
        serde_json::to_string_pretty(records).map_err(|error| ImportModelError::ArtifactWrite {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
    fs::write(path, output).map_err(|error| ImportModelError::ArtifactWrite {
        path: path.display().to_string(),
        message: error.to_string(),
    })
}

fn update_counts(
    status: ImportRecordStatus,
    imported_records: &mut usize,
    updated_records: &mut usize,
    skipped_records: &mut usize,
    staged_records: &mut usize,
    failed_records: &mut usize,
) {
    match status {
        ImportRecordStatus::Imported => *imported_records += 1,
        ImportRecordStatus::Updated => *updated_records += 1,
        ImportRecordStatus::SkippedUnchanged => *skipped_records += 1,
        ImportRecordStatus::StagedForReview => *staged_records += 1,
        ImportRecordStatus::FailedValidation => *failed_records += 1,
    }
}

fn validation_mode_label(mode: ValidationMode) -> &'static str {
    match mode {
        ValidationMode::Strict => "strict",
        ValidationMode::Permissive => "permissive",
    }
}

fn publication_mode_label(mode: PublicationMode) -> &'static str {
    match mode {
        PublicationMode::ValidateOnly => "validate_only",
        PublicationMode::StageValidated => "stage_validated",
        PublicationMode::PublishValidated => "publish_validated",
    }
}

fn publication_state_label(mode: PublicationMode) -> &'static str {
    match mode {
        PublicationMode::PublishValidated => "published",
        PublicationMode::StageValidated | PublicationMode::ValidateOnly => "staged",
    }
}

fn asset_storage_default_label(mode: AssetStorageDefault) -> &'static str {
    match mode {
        AssetStorageDefault::PublicUpload => "public_upload",
        AssetStorageDefault::PrivateShared => "private_shared",
        AssetStorageDefault::LocalOnlySensitive => "local_only_sensitive",
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
