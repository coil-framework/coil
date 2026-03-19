use super::*;

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
