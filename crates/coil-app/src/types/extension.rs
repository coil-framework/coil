use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerExtension {
    pub id: ExtensionId,
    pub package_version: ContractVersion,
    pub artifact_sha256: String,
    pub config: BTreeMap<String, ExtensionConfigValue>,
    pub installation: ExtensionInstallation,
}

impl CustomerExtension {
    pub fn new(
        id: impl Into<String>,
        package_version: ContractVersion,
        artifact_sha256: impl Into<String>,
        installation: ExtensionInstallation,
    ) -> Result<Self, AppModelError> {
        Ok(Self {
            id: ExtensionId::new(id.into())?,
            package_version,
            artifact_sha256: validate_sha256("extension_artifact_sha256", artifact_sha256.into())?,
            config: BTreeMap::new(),
            installation,
        })
    }

    pub fn with_config_value(
        mut self,
        key: impl Into<String>,
        value: ExtensionConfigValue,
    ) -> Result<Self, AppModelError> {
        let key = validate_token("extension_config_key", key.into())?;
        self.config.insert(key, value);
        Ok(self)
    }
}
