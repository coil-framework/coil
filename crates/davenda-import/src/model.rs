use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use davenda_cli::{CommandReport, DiagnosticRecord, DiagnosticSeverity, ReportRow, ReportStatus};

use super::validation::{require_non_empty, validate_token, ImportModelError};

macro_rules! token_type {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, ImportModelError> {
                Ok(Self(validate_token($field, value.into())?))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

token_type!(ImportRunId, "import_run_id");
token_type!(SourceSystemId, "source_system_id");
token_type!(ImporterId, "importer_id");
token_type!(SourceRecordKey, "source_record_key");
token_type!(TargetRecordId, "target_record_id");
token_type!(RollbackTriggerId, "rollback_trigger_id");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationMode {
    Strict,
    Permissive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationMode {
    ValidateOnly,
    StageValidated,
    PublishValidated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetStorageDefault {
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImporterSpec {
    pub id: ImporterId,
    pub phase: u16,
    pub resource_kind: String,
    pub description: String,
    pub dependencies: Vec<ImporterId>,
}

impl ImporterSpec {
    pub fn new(
        id: ImporterId,
        phase: u16,
        resource_kind: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            id,
            phase,
            resource_kind: require_non_empty("resource_kind", resource_kind.into())?,
            description: require_non_empty("importer_description", description.into())?,
            dependencies: Vec::new(),
        })
    }

    pub fn depending_on(mut self, dependency: ImporterId) -> Self {
        self.dependencies.push(dependency);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportManifest {
    pub run_id: ImportRunId,
    pub source_system: SourceSystemId,
    pub snapshot_at: String,
    pub customer_app_id: String,
    pub modules: Vec<String>,
    pub locale: Option<String>,
    pub site: Option<String>,
    pub validation_mode: ValidationMode,
    pub publication_mode: PublicationMode,
    pub asset_storage_default: AssetStorageDefault,
    pub importers: Vec<ImporterSpec>,
}

impl ImportManifest {
    pub fn new(
        run_id: ImportRunId,
        source_system: SourceSystemId,
        snapshot_at: impl Into<String>,
        customer_app_id: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            run_id,
            source_system,
            snapshot_at: require_non_empty("snapshot_at", snapshot_at.into())?,
            customer_app_id: validate_token("customer_app_id", customer_app_id.into())?,
            modules: Vec::new(),
            locale: None,
            site: None,
            validation_mode: ValidationMode::Strict,
            publication_mode: PublicationMode::StageValidated,
            asset_storage_default: AssetStorageDefault::PublicUpload,
            importers: Vec::new(),
        })
    }

    pub fn with_module(mut self, module: impl Into<String>) -> Result<Self, ImportModelError> {
        self.modules
            .push(validate_token("module_name", module.into())?);
        Ok(self)
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Result<Self, ImportModelError> {
        self.locale = Some(require_non_empty("locale", locale.into())?);
        Ok(self)
    }

    pub fn with_site(mut self, site: impl Into<String>) -> Result<Self, ImportModelError> {
        self.site = Some(validate_token("site_id", site.into())?);
        Ok(self)
    }

    pub fn with_importer(mut self, importer: ImporterSpec) -> Self {
        self.importers.push(importer);
        self
    }

    pub fn validate(&self) -> Result<(), ImportModelError> {
        let mut seen = BTreeSet::new();
        for importer in &self.importers {
            if !seen.insert(importer.id.clone()) {
                return Err(ImportModelError::DuplicateImporter {
                    importer_id: importer.id.to_string(),
                });
            }

            for dependency in &importer.dependencies {
                if dependency == &importer.id {
                    return Err(ImportModelError::SelfDependency {
                        importer_id: importer.id.to_string(),
                    });
                }

                if !self
                    .importers
                    .iter()
                    .any(|candidate| &candidate.id == dependency)
                {
                    return Err(ImportModelError::UnknownImporterDependency {
                        importer_id: importer.id.to_string(),
                        dependency: dependency.to_string(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn plan(&self) -> Result<ImportPlan, ImportModelError> {
        self.validate()?;

        let mut indegree = self
            .importers
            .iter()
            .map(|importer| (importer.id.clone(), 0usize))
            .collect::<BTreeMap<_, _>>();
        let mut dependents = self
            .importers
            .iter()
            .map(|importer| (importer.id.clone(), Vec::<ImporterId>::new()))
            .collect::<BTreeMap<_, _>>();

        for importer in &self.importers {
            for dependency in &importer.dependencies {
                *indegree.get_mut(&importer.id).expect("importer exists") += 1;
                dependents
                    .get_mut(dependency)
                    .expect("dependency exists")
                    .push(importer.id.clone());
            }
        }

        let importer_by_id = self
            .importers
            .iter()
            .map(|importer| (importer.id.clone(), importer.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut queue = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        queue.sort_by(|left, right| {
            importer_by_id[left]
                .phase
                .cmp(&importer_by_id[right].phase)
                .then(left.as_str().cmp(right.as_str()))
        });
        let mut queue = VecDeque::from(queue);
        let mut ordered = Vec::new();

        while let Some(importer_id) = queue.pop_front() {
            let importer = importer_by_id
                .get(&importer_id)
                .expect("queued importer exists")
                .clone();
            ordered.push(importer.clone());

            for dependent in dependents.get(&importer_id).into_iter().flatten() {
                let degree = indegree.get_mut(dependent).expect("dependent exists");
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(dependent.clone());
                }
            }
        }

        if ordered.len() != self.importers.len() {
            return Err(ImportModelError::CyclicImporterDependencies);
        }

        ordered.sort_by(|left, right| {
            left.phase
                .cmp(&right.phase)
                .then(left.id.as_str().cmp(right.id.as_str()))
        });

        Ok(ImportPlan {
            run_id: self.run_id.clone(),
            customer_app_id: self.customer_app_id.clone(),
            ordered_importers: ordered,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub run_id: ImportRunId,
    pub customer_app_id: String,
    pub ordered_importers: Vec<ImporterSpec>,
}

impl ImportPlan {
    pub fn command_report(&self) -> Result<CommandReport, ImportModelError> {
        let mut report = CommandReport::new(
            ["import", "run"],
            format!(
                "Planned import run `{}` for `{}`",
                self.run_id, self.customer_app_id
            ),
        )?
        .with_columns([
            "phase",
            "importer",
            "resource_kind",
            "dependencies",
            "description",
        ])?;

        for importer in &self.ordered_importers {
            report.push_row(
                ReportRow::new()
                    .with_cell("phase", importer.phase.to_string())?
                    .with_cell("importer", importer.id.to_string())?
                    .with_cell("resource_kind", importer.resource_kind.clone())?
                    .with_cell(
                        "dependencies",
                        if importer.dependencies.is_empty() {
                            "none".to_string()
                        } else {
                            importer
                                .dependencies
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(",")
                        },
                    )?
                    .with_cell("description", importer.description.clone())?,
            );
        }

        Ok(report)
    }
}

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
