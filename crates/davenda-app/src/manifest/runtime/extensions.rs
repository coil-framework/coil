use super::*;

impl CustomerAppManifest {
    pub(super) fn resolve_extension_packages(
        &self,
        packages: &[ExtensionPackage],
    ) -> Result<Vec<davenda_wasm::InstalledExtension>, AppModelError> {
        if self.extensions.is_empty() {
            return Ok(Vec::new());
        }

        if packages.is_empty() {
            return Err(AppModelError::ExtensionPackagesRequired {
                app_id: self.id.to_string(),
            });
        }

        let mut installed = Vec::new();
        for extension in &self.extensions {
            let package = packages
                .iter()
                .find(|package| package.id().as_str() == extension.id.as_str())
                .ok_or_else(|| AppModelError::UnknownExtensionPackage {
                    app_id: self.id.to_string(),
                    extension_id: extension.id.to_string(),
                })?;

            if package.version() != extension.package_version {
                return Err(AppModelError::ExtensionVersionMismatch {
                    extension_id: extension.id.to_string(),
                    configured: extension.package_version,
                    actual: package.version(),
                });
            }

            if package.artifact_sha256 != extension.artifact_sha256 {
                return Err(AppModelError::ExtensionArtifactChecksumMismatch {
                    extension_id: extension.id.to_string(),
                    configured: extension.artifact_sha256.clone(),
                    actual: package.artifact_sha256.clone(),
                });
            }

            installed.push(package.install(extension.installation.clone(), &extension.config)?);
        }

        Ok(installed)
    }
}
