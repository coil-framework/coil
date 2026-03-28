use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::{
    AssetStorageDefault, ImportCutover, ImportCutoverTrigger, ImportManifest,
    ImportMigrationArtifacts, ImportModelError, ImportRunId, ImportSource, ImportSourceFormat,
    ImportSourceInput, ImportTarget, ImportVerification, ImportWebhookVerification, ImporterId,
    ImporterSpec, PublicationMode, RollbackTriggerId, SourceSystemId, ValidationMode,
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportManifestDocument {
    pub run_id: String,
    pub source_system: String,
    pub snapshot_at: String,
    pub customer_app_id: String,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub validation_mode: DocumentValidationMode,
    #[serde(default)]
    pub publication_mode: DocumentPublicationMode,
    #[serde(default)]
    pub asset_storage_default: DocumentAssetStorageDefault,
    #[serde(default)]
    pub target: Option<ImportTargetDocument>,
    #[serde(default)]
    pub source: Option<ImportSourceDocument>,
    #[serde(default)]
    pub migration_artifacts: Option<ImportMigrationArtifactsDocument>,
    #[serde(default)]
    pub verification: Option<ImportVerificationDocument>,
    #[serde(default)]
    pub cutover: Option<ImportCutoverDocument>,
    #[serde(default)]
    pub importers: Vec<ImporterDocument>,
}

impl ImportManifestDocument {
    pub fn from_toml_str(input: &str) -> Result<Self, ImportModelError> {
        toml::from_str(input).map_err(|error| ImportModelError::ManifestParse {
            message: error.to_string(),
        })
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ImportModelError> {
        let path = path.as_ref();
        let input = fs::read_to_string(path).map_err(|error| ImportModelError::ManifestRead {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Self::from_toml_str(&input)
    }

    pub fn into_manifest(self) -> Result<ImportManifest, ImportModelError> {
        let mut manifest = ImportManifest::new(
            ImportRunId::new(self.run_id)?,
            SourceSystemId::new(self.source_system)?,
            self.snapshot_at,
            self.customer_app_id,
        )?;
        manifest.validation_mode = self.validation_mode.into();
        manifest.publication_mode = self.publication_mode.into();
        manifest.asset_storage_default = self.asset_storage_default.into();
        if let Some(target) = self.target {
            manifest = manifest.with_target(target.into_model()?);
        }
        if let Some(source) = self.source {
            manifest = manifest.with_source(source.into_model()?);
        }
        if let Some(artifacts) = self.migration_artifacts {
            manifest = manifest.with_migration_artifacts(artifacts.into_model()?);
        }
        if let Some(verification) = self.verification {
            manifest = manifest.with_verification(verification.into_model()?);
        }
        if let Some(cutover) = self.cutover {
            manifest = manifest.with_cutover(cutover.into_model()?);
        }

        for module in self.modules {
            manifest = manifest.with_module(module)?;
        }

        if let Some(locale) = self.locale {
            manifest = manifest.with_locale(locale)?;
        }

        if let Some(site) = self.site {
            manifest = manifest.with_site(site)?;
        }

        for importer in self.importers {
            manifest = manifest.with_importer(importer.into_spec()?);
        }

        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImporterDocument {
    pub id: String,
    pub phase: u16,
    pub resource_kind: String,
    pub description: String,
    #[serde(default)]
    pub source_path: Option<String>,
    #[serde(default)]
    pub source_format: DocumentImportSourceFormat,
    #[serde(default)]
    pub mapping: BTreeMap<String, String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportTargetDocument {
    pub app_manifest: String,
    pub platform_config: String,
    #[serde(default)]
    pub expected_modules: Vec<String>,
}

impl ImportTargetDocument {
    fn into_model(self) -> Result<ImportTarget, ImportModelError> {
        let mut target = ImportTarget::new(self.app_manifest, self.platform_config)?;
        for module in self.expected_modules {
            target = target.with_expected_module(module)?;
        }
        Ok(target)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportSourceInputDocument {
    pub id: String,
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub checksum: Option<String>,
}

impl ImportSourceInputDocument {
    fn into_model(self) -> Result<ImportSourceInput, ImportModelError> {
        let mut input = ImportSourceInput::new(self.id, self.kind, self.path)?;
        if let Some(checksum) = self.checksum {
            input = input.with_checksum(checksum)?;
        }
        Ok(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportSourceDocument {
    pub kind: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub snapshot_id: Option<String>,
    #[serde(default)]
    pub inputs: Vec<ImportSourceInputDocument>,
}

impl ImportSourceDocument {
    fn into_model(self) -> Result<ImportSource, ImportModelError> {
        let mut source = ImportSource::new(self.kind)?;
        if let Some(base_url) = self.base_url {
            source = source.with_base_url(base_url)?;
        }
        if let Some(timezone) = self.timezone {
            source = source.with_timezone(timezone)?;
        }
        if let Some(snapshot_id) = self.snapshot_id {
            source = source.with_snapshot_id(snapshot_id)?;
        }
        for input in self.inputs {
            source = source.with_input(input.into_model()?);
        }
        Ok(source)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportMigrationArtifactsDocument {
    pub capability_map: String,
    pub auth_mapping: String,
    pub redirect_plan: String,
    pub extraction_spec: String,
    pub cutover_runbook: String,
}

impl ImportMigrationArtifactsDocument {
    fn into_model(self) -> Result<ImportMigrationArtifacts, ImportModelError> {
        ImportMigrationArtifacts::new(
            self.capability_map,
            self.auth_mapping,
            self.redirect_plan,
            self.extraction_spec,
            self.cutover_runbook,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ImportVerificationDocument {
    #[serde(default)]
    pub required: Vec<String>,
    #[serde(default)]
    pub sample_routes: Vec<String>,
    #[serde(default)]
    pub sample_users: Vec<String>,
    #[serde(default)]
    pub webhooks: Vec<ImportWebhookVerificationDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportWebhookVerificationDocument {
    pub source: String,
    pub event: String,
    #[serde(default)]
    pub max_verification_failures: u32,
    #[serde(default)]
    pub max_replay_rejections: u32,
}

impl ImportVerificationDocument {
    fn into_model(self) -> Result<ImportVerification, ImportModelError> {
        let mut verification = ImportVerification::default();
        for required in self.required {
            verification = verification.with_required(required)?;
        }
        for route in self.sample_routes {
            verification = verification.with_sample_route(route)?;
        }
        for user in self.sample_users {
            verification = verification.with_sample_user(user)?;
        }
        for webhook in self.webhooks {
            verification = verification.with_webhook(webhook.into_model()?);
        }
        Ok(verification)
    }
}

impl ImportWebhookVerificationDocument {
    fn into_model(self) -> Result<ImportWebhookVerification, ImportModelError> {
        Ok(ImportWebhookVerification::new(self.source, self.event)?
            .with_max_verification_failures(self.max_verification_failures)
            .with_max_replay_rejections(self.max_replay_rejections))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ImportCutoverTriggerDocument {
    pub id: String,
    pub description: String,
}

impl ImportCutoverTriggerDocument {
    fn into_model(self) -> Result<ImportCutoverTrigger, ImportModelError> {
        ImportCutoverTrigger::new(RollbackTriggerId::new(self.id)?, self.description)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct ImportCutoverDocument {
    #[serde(default)]
    pub freeze_legacy_writes: bool,
    #[serde(default)]
    pub switch_method: Option<String>,
    #[serde(default)]
    pub hostnames: Vec<String>,
    #[serde(default)]
    pub requires_assets_publish: bool,
    #[serde(default)]
    pub requires_migrate_apply: bool,
    #[serde(default)]
    pub requires_storage_validation: bool,
    #[serde(default)]
    pub requires_cache_warm: bool,
    #[serde(default)]
    pub observation_window_minutes: Option<u32>,
    #[serde(default)]
    pub rollback_triggers: Vec<ImportCutoverTriggerDocument>,
}

impl ImportCutoverDocument {
    fn into_model(self) -> Result<ImportCutover, ImportModelError> {
        let mut cutover = ImportCutover {
            freeze_legacy_writes: self.freeze_legacy_writes,
            requires_assets_publish: self.requires_assets_publish,
            requires_migrate_apply: self.requires_migrate_apply,
            requires_storage_validation: self.requires_storage_validation,
            requires_cache_warm: self.requires_cache_warm,
            ..ImportCutover::default()
        };
        if let Some(method) = self.switch_method {
            cutover = cutover.with_switch_method(method)?;
        }
        if let Some(minutes) = self.observation_window_minutes {
            cutover = cutover.with_observation_window(minutes);
        }
        for hostname in self.hostnames {
            cutover = cutover.with_hostname(hostname)?;
        }
        for trigger in self.rollback_triggers {
            cutover = cutover.with_trigger(trigger.into_model()?);
        }
        Ok(cutover)
    }
}

impl ImporterDocument {
    fn into_spec(self) -> Result<ImporterSpec, ImportModelError> {
        let mut importer = ImporterSpec::new(
            ImporterId::new(self.id)?,
            self.phase,
            self.resource_kind,
            self.description,
        )?;

        if let Some(source_path) = self.source_path {
            importer = importer.with_source_path(source_path)?;
        }
        importer = importer.with_source_format(self.source_format.into());
        for (key, value) in self.mapping {
            importer = importer.with_mapping(key, value)?;
        }
        for dependency in self.dependencies {
            importer = importer.depending_on(ImporterId::new(dependency)?);
        }

        Ok(importer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentValidationMode {
    #[default]
    Strict,
    Permissive,
}

impl From<DocumentValidationMode> for ValidationMode {
    fn from(value: DocumentValidationMode) -> Self {
        match value {
            DocumentValidationMode::Strict => ValidationMode::Strict,
            DocumentValidationMode::Permissive => ValidationMode::Permissive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentPublicationMode {
    ValidateOnly,
    #[default]
    StageValidated,
    PublishValidated,
}

impl From<DocumentPublicationMode> for PublicationMode {
    fn from(value: DocumentPublicationMode) -> Self {
        match value {
            DocumentPublicationMode::ValidateOnly => PublicationMode::ValidateOnly,
            DocumentPublicationMode::StageValidated => PublicationMode::StageValidated,
            DocumentPublicationMode::PublishValidated => PublicationMode::PublishValidated,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentAssetStorageDefault {
    #[default]
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
}

impl From<DocumentAssetStorageDefault> for AssetStorageDefault {
    fn from(value: DocumentAssetStorageDefault) -> Self {
        match value {
            DocumentAssetStorageDefault::PublicUpload => AssetStorageDefault::PublicUpload,
            DocumentAssetStorageDefault::PrivateShared => AssetStorageDefault::PrivateShared,
            DocumentAssetStorageDefault::LocalOnlySensitive => {
                AssetStorageDefault::LocalOnlySensitive
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DocumentImportSourceFormat {
    #[default]
    Json,
}

impl From<DocumentImportSourceFormat> for ImportSourceFormat {
    fn from(value: DocumentImportSourceFormat) -> Self {
        match value {
            DocumentImportSourceFormat::Json => ImportSourceFormat::Json,
        }
    }
}
