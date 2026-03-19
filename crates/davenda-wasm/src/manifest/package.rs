use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionPackage {
    pub publisher: String,
    pub manifest: ExtensionManifest,
    pub artifact_source: ExtensionArtifactSource,
    pub artifact_sha256: String,
    pub config_schema: ExtensionConfigSchema,
}

impl ExtensionPackage {
    pub fn new(
        publisher: impl Into<String>,
        manifest: ExtensionManifest,
        artifact_source: ExtensionArtifactSource,
        artifact_sha256: impl Into<String>,
        config_schema: ExtensionConfigSchema,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            publisher: require_non_empty("extension_publisher", publisher.into())?,
            manifest,
            artifact_source,
            artifact_sha256: validate_sha256("extension_artifact_sha256", artifact_sha256.into())?,
            config_schema,
        })
    }

    pub fn install(
        &self,
        installation: ExtensionInstallation,
        configured_values: &std::collections::BTreeMap<String, ExtensionConfigValue>,
    ) -> Result<InstalledExtension, WasmModelError> {
        let mut installed = InstalledExtension::install(self.manifest.clone(), installation)?;
        installed.config = self.config_schema.effective_values(configured_values)?;
        installed.artifact = Some(InstalledArtifact::new(
            self.publisher.clone(),
            self.artifact_source.clone(),
            self.artifact_sha256.clone(),
        )?);
        Ok(installed)
    }

    pub fn id(&self) -> &ExtensionId {
        &self.manifest.id
    }

    pub fn version(&self) -> ContractVersion {
        self.manifest.version
    }
}
