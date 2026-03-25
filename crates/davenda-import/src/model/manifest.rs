use super::*;
use std::path::Path;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSourceFormat {
    Json,
}

impl ImportSourceFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportTarget {
    pub app_manifest: String,
    pub platform_config: String,
    pub expected_modules: Vec<String>,
}

impl ImportTarget {
    pub fn new(
        app_manifest: impl Into<String>,
        platform_config: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            app_manifest: require_non_empty("target_app_manifest", app_manifest.into())?,
            platform_config: require_non_empty("target_platform_config", platform_config.into())?,
            expected_modules: Vec::new(),
        })
    }

    pub fn with_expected_module(
        mut self,
        module: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        self.expected_modules
            .push(validate_token("expected_module", module.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSourceInput {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub checksum: Option<String>,
}

impl ImportSourceInput {
    pub fn new(
        id: impl Into<String>,
        kind: impl Into<String>,
        path: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            id: validate_token("source_input_id", id.into())?,
            kind: validate_token("source_input_kind", kind.into())?,
            path: require_non_empty("source_input_path", path.into())?,
            checksum: None,
        })
    }

    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Result<Self, ImportModelError> {
        self.checksum = Some(require_non_empty("source_input_checksum", checksum.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSource {
    pub kind: String,
    pub base_url: Option<String>,
    pub timezone: Option<String>,
    pub snapshot_id: Option<String>,
    pub inputs: Vec<ImportSourceInput>,
}

impl ImportSource {
    pub fn new(kind: impl Into<String>) -> Result<Self, ImportModelError> {
        Ok(Self {
            kind: validate_token("source_kind", kind.into())?,
            base_url: None,
            timezone: None,
            snapshot_id: None,
            inputs: Vec::new(),
        })
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Result<Self, ImportModelError> {
        self.base_url = Some(require_non_empty("source_base_url", base_url.into())?);
        Ok(self)
    }

    pub fn with_timezone(mut self, timezone: impl Into<String>) -> Result<Self, ImportModelError> {
        self.timezone = Some(require_non_empty("source_timezone", timezone.into())?);
        Ok(self)
    }

    pub fn with_snapshot_id(
        mut self,
        snapshot_id: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        self.snapshot_id = Some(require_non_empty("source_snapshot_id", snapshot_id.into())?);
        Ok(self)
    }

    pub fn with_input(mut self, input: ImportSourceInput) -> Self {
        self.inputs.push(input);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportMigrationArtifacts {
    pub capability_map: String,
    pub auth_mapping: String,
    pub redirect_plan: String,
    pub extraction_spec: String,
    pub cutover_runbook: String,
}

impl ImportMigrationArtifacts {
    pub fn new(
        capability_map: impl Into<String>,
        auth_mapping: impl Into<String>,
        redirect_plan: impl Into<String>,
        extraction_spec: impl Into<String>,
        cutover_runbook: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            capability_map: require_non_empty("capability_map", capability_map.into())?,
            auth_mapping: require_non_empty("auth_mapping", auth_mapping.into())?,
            redirect_plan: require_non_empty("redirect_plan", redirect_plan.into())?,
            extraction_spec: require_non_empty("extraction_spec", extraction_spec.into())?,
            cutover_runbook: require_non_empty("cutover_runbook", cutover_runbook.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportVerification {
    pub required: Vec<String>,
    pub sample_routes: Vec<String>,
    pub sample_users: Vec<String>,
}

impl ImportVerification {
    pub fn with_required(mut self, check: impl Into<String>) -> Result<Self, ImportModelError> {
        self.required
            .push(validate_token("verification_check", check.into())?);
        Ok(self)
    }

    pub fn with_sample_route(
        mut self,
        route: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        self.sample_routes
            .push(require_non_empty("verification_sample_route", route.into())?);
        Ok(self)
    }

    pub fn with_sample_user(
        mut self,
        user: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        self.sample_users
            .push(require_non_empty("verification_sample_user", user.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCutoverTrigger {
    pub id: RollbackTriggerId,
    pub description: String,
}

impl ImportCutoverTrigger {
    pub fn new(
        id: RollbackTriggerId,
        description: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        Ok(Self {
            id,
            description: require_non_empty("cutover_trigger_description", description.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportCutover {
    pub freeze_legacy_writes: bool,
    pub switch_method: Option<String>,
    pub hostnames: Vec<String>,
    pub requires_assets_publish: bool,
    pub requires_migrate_apply: bool,
    pub requires_storage_validation: bool,
    pub requires_cache_warm: bool,
    pub observation_window_minutes: Option<u32>,
    pub rollback_triggers: Vec<ImportCutoverTrigger>,
}

impl ImportCutover {
    pub fn with_switch_method(
        mut self,
        switch_method: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        self.switch_method = Some(validate_token("cutover_switch_method", switch_method.into())?);
        Ok(self)
    }

    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Result<Self, ImportModelError> {
        self.hostnames
            .push(require_non_empty("cutover_hostname", hostname.into())?);
        Ok(self)
    }

    pub fn with_observation_window(mut self, minutes: u32) -> Self {
        self.observation_window_minutes = Some(minutes);
        self
    }

    pub fn with_trigger(mut self, trigger: ImportCutoverTrigger) -> Self {
        self.rollback_triggers.push(trigger);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImporterSpec {
    pub id: ImporterId,
    pub phase: u16,
    pub resource_kind: String,
    pub description: String,
    pub source_path: Option<String>,
    pub source_format: ImportSourceFormat,
    pub mapping: BTreeMap<String, String>,
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
            source_path: None,
            source_format: ImportSourceFormat::Json,
            mapping: BTreeMap::new(),
            dependencies: Vec::new(),
        })
    }

    pub fn with_source_path(
        mut self,
        source_path: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        self.source_path = Some(require_non_empty("source_path", source_path.into())?);
        Ok(self)
    }

    pub fn with_source_format(mut self, source_format: ImportSourceFormat) -> Self {
        self.source_format = source_format;
        self
    }

    pub fn with_mapping(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, ImportModelError> {
        let key = validate_token("mapping_key", key.into())?;
        let value = require_non_empty("mapping_value", value.into())?;
        self.mapping.insert(key, value);
        Ok(self)
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
    pub target: Option<ImportTarget>,
    pub source: Option<ImportSource>,
    pub migration_artifacts: Option<ImportMigrationArtifacts>,
    pub verification: Option<ImportVerification>,
    pub cutover: Option<ImportCutover>,
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
            target: None,
            source: None,
            migration_artifacts: None,
            verification: None,
            cutover: None,
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

    pub fn with_target(mut self, target: ImportTarget) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_source(mut self, source: ImportSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn with_migration_artifacts(mut self, artifacts: ImportMigrationArtifacts) -> Self {
        self.migration_artifacts = Some(artifacts);
        self
    }

    pub fn with_verification(mut self, verification: ImportVerification) -> Self {
        self.verification = Some(verification);
        self
    }

    pub fn with_cutover(mut self, cutover: ImportCutover) -> Self {
        self.cutover = Some(cutover);
        self
    }

    pub fn from_toml_str(input: &str) -> Result<Self, ImportModelError> {
        crate::ImportManifestDocument::from_toml_str(input)?.into_manifest()
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ImportModelError> {
        crate::ImportManifestDocument::from_file(path)?.into_manifest()
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
            source_system: self.source_system.clone(),
            customer_app_id: self.customer_app_id.clone(),
            locale: self.locale.clone(),
            site: self.site.clone(),
            validation_mode: self.validation_mode,
            publication_mode: self.publication_mode,
            asset_storage_default: self.asset_storage_default,
            ordered_importers: ordered,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPlan {
    pub run_id: ImportRunId,
    pub source_system: SourceSystemId,
    pub customer_app_id: String,
    pub locale: Option<String>,
    pub site: Option<String>,
    pub validation_mode: ValidationMode,
    pub publication_mode: PublicationMode,
    pub asset_storage_default: AssetStorageDefault,
    pub ordered_importers: Vec<ImporterSpec>,
}

impl ImportPlan {
    pub fn execute(
        &self,
        manifest_root: impl AsRef<Path>,
        journal_path: impl AsRef<Path>,
    ) -> Result<ImportExecution, ImportModelError> {
        self.execute_with_handler(
            manifest_root,
            journal_path,
            |_, _, _, _| Ok(()),
        )
    }

    pub fn execute_with_handler<F>(
        &self,
        manifest_root: impl AsRef<Path>,
        journal_path: impl AsRef<Path>,
        handler: F,
    ) -> Result<ImportExecution, ImportModelError>
    where
        F: FnMut(&ImporterSpec, &ImportRecordReceipt, &Path, &mut serde_json::Value) -> Result<(), ImportModelError>,
    {
        super::execute_import_plan_with_handler(
            self,
            manifest_root.as_ref(),
            journal_path.as_ref(),
            handler,
        )
    }

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
            "source",
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
                        "source",
                        importer
                            .source_path
                            .clone()
                            .unwrap_or_else(|| "missing".to_string()),
                    )?
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
