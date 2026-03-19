use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::{
    AssetStorageDefault, ImportManifest, ImportModelError, ImportRunId, ImporterId, ImporterSpec,
    PublicationMode, SourceSystemId, ValidationMode,
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
    pub dependencies: Vec<String>,
}

impl ImporterDocument {
    fn into_spec(self) -> Result<ImporterSpec, ImportModelError> {
        let mut importer = ImporterSpec::new(
            ImporterId::new(self.id)?,
            self.phase,
            self.resource_kind,
            self.description,
        )?;

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
            DocumentAssetStorageDefault::LocalOnlySensitive => AssetStorageDefault::LocalOnlySensitive,
        }
    }
}
