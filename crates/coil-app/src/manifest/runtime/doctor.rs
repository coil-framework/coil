use super::*;

impl CustomerAppManifest {
    pub fn release_doctor_with_extensions<P>(
        &self,
        auth_package: &P,
        manifests: &[ModuleManifest],
        packages: &[ExtensionPackage],
        config: Option<&PlatformConfig>,
    ) -> Result<ReleaseDoctorReport, AppModelError>
    where
        P: AuthModelPackage + 'static,
    {
        let composition = self.compose(auth_package, manifests)?;
        let mut report = composition.release_doctor(config);
        self.append_extension_doctor_findings(packages, &mut report);
        Ok(report)
    }

    fn append_extension_doctor_findings(
        &self,
        packages: &[ExtensionPackage],
        report: &mut ReleaseDoctorReport,
    ) {
        if self.extensions.is_empty() {
            return;
        }

        if packages.is_empty() {
            report.findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "extension.packages.missing",
                format!(
                    "customer app `{}` installs extensions but no extension packages were supplied",
                    self.id
                ),
            ));
            return;
        }

        for extension in &self.extensions {
            let Some(package) = packages
                .iter()
                .find(|package| package.id().as_str() == extension.id.as_str())
            else {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.package.unknown",
                    format!(
                        "customer extension `{}` does not have a matching package artifact",
                        extension.id
                    ),
                ));
                continue;
            };

            if package.version() != extension.package_version {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.version.mismatch",
                    format!(
                        "customer extension `{}` pins version `{}` but package provides `{}`",
                        extension.id,
                        extension.package_version,
                        package.version()
                    ),
                ));
            }

            if package.artifact_sha256 != extension.artifact_sha256 {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.checksum.mismatch",
                    format!(
                        "customer extension `{}` pins digest `{}` but package provides `{}`",
                        extension.id, extension.artifact_sha256, package.artifact_sha256
                    ),
                ));
            }

            if let Err(error) = package.config_schema.effective_values(&extension.config) {
                report.findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Blocking,
                    "extension.config.invalid",
                    format!(
                        "customer extension `{}` has invalid config: {error}",
                        extension.id
                    ),
                ));
            }
        }
    }
}
