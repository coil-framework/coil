use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseDoctorSeverity {
    Info,
    Warning,
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDoctorFinding {
    pub severity: ReleaseDoctorSeverity,
    pub code: String,
    pub message: String,
}

impl ReleaseDoctorFinding {
    pub fn new(
        severity: ReleaseDoctorSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseDoctorReport {
    pub app_id: CustomerAppId,
    pub findings: Vec<ReleaseDoctorFinding>,
}

impl ReleaseDoctorReport {
    pub fn is_compatible(&self) -> bool {
        !self
            .findings
            .iter()
            .any(|finding| finding.severity == ReleaseDoctorSeverity::Blocking)
    }

    pub fn blocking_findings(&self) -> impl Iterator<Item = &ReleaseDoctorFinding> {
        self.findings
            .iter()
            .filter(|finding| finding.severity == ReleaseDoctorSeverity::Blocking)
    }

    pub fn command_report(&self) -> Result<CommandReport, AppModelError> {
        let mut report = CommandReport::new(
            ["release", "doctor"],
            format!(
                "Checked upgrade compatibility for customer app `{}`",
                self.app_id
            ),
        )?
        .with_columns(["severity", "code", "message"])?;
        report = report.with_status(
            match self
                .findings
                .iter()
                .map(|finding| finding.severity)
                .max_by_key(|severity| release_doctor_rank(*severity))
            {
                Some(ReleaseDoctorSeverity::Blocking) => ReportStatus::Unsafe,
                Some(ReleaseDoctorSeverity::Warning) => ReportStatus::Warning,
                _ => ReportStatus::Ok,
            },
        );

        for finding in &self.findings {
            report.push_row(
                ReportRow::new()
                    .with_cell("severity", release_doctor_label(finding.severity))?
                    .with_cell("code", finding.code.clone())?
                    .with_cell("message", finding.message.clone())?,
            );
            report.push_diagnostic(DiagnosticRecord::new(
                match finding.severity {
                    ReleaseDoctorSeverity::Info => DiagnosticSeverity::Info,
                    ReleaseDoctorSeverity::Warning => DiagnosticSeverity::Warning,
                    ReleaseDoctorSeverity::Blocking => DiagnosticSeverity::Error,
                },
                finding.code.clone(),
                finding.message.clone(),
            )?);
        }

        Ok(report)
    }
}

pub(crate) fn config_alignment_findings(
    composition: &CustomerAppComposition,
    config: &PlatformConfig,
) -> Vec<ReleaseDoctorFinding> {
    let mut findings = Vec::new();

    if config.app.name != composition.app_id.as_str() {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.app.mismatch",
            format!(
                "runtime config app `{}` does not match customer app manifest `{}`",
                config.app.name, composition.app_id
            ),
        ));
    }

    if config.auth.package != composition.auth.package_name {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.auth_package.mismatch",
            format!(
                "runtime config auth package `{}` does not match customer app auth package `{}`",
                config.auth.package, composition.auth.package_name
            ),
        ));
    }

    if config.i18n.default_locale != composition.default_locale.as_str() {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.i18n.default_locale",
            format!(
                "runtime config default locale `{}` does not match customer app default locale `{}`",
                config.i18n.default_locale, composition.default_locale
            ),
        ));
    }

    let manifest_locales = sorted_locale_strings(&composition.supported_locales);
    let configured_locales = sorted_strings(config.i18n.supported_locales.clone());
    if manifest_locales != configured_locales {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.i18n.supported_locales",
            format!(
                "runtime config supported locales {:?} do not match customer app locales {:?}",
                configured_locales, manifest_locales
            ),
        ));
    }

    if let Some(canonical_domain) = composition.canonical_domain() {
        if config.seo.canonical_host != canonical_domain {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "config.seo.canonical_host",
                format!(
                    "runtime config canonical host `{}` does not match customer app canonical domain `{}`",
                    config.seo.canonical_host, canonical_domain
                ),
            ));
        }
    }

    let manifest_modules = sorted_strings(
        composition
            .installed_modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>(),
    );
    let configured_modules = sorted_strings(config.modules.enabled.clone());
    let manifest_only = difference(&manifest_modules, &configured_modules);
    let configured_only = difference(&configured_modules, &manifest_modules);
    if !manifest_only.is_empty() || !configured_only.is_empty() {
        findings.push(ReleaseDoctorFinding::new(
            ReleaseDoctorSeverity::Blocking,
            "config.modules.enabled",
            format!(
                "runtime config modules drift from the customer app manifest; manifest-only={manifest_only:?}, config-only={configured_only:?}"
            ),
        ));
    }

    findings
}

fn release_doctor_rank(severity: ReleaseDoctorSeverity) -> u8 {
    match severity {
        ReleaseDoctorSeverity::Info => 0,
        ReleaseDoctorSeverity::Warning => 1,
        ReleaseDoctorSeverity::Blocking => 2,
    }
}

fn release_doctor_label(severity: ReleaseDoctorSeverity) -> &'static str {
    match severity {
        ReleaseDoctorSeverity::Info => "info",
        ReleaseDoctorSeverity::Warning => "warning",
        ReleaseDoctorSeverity::Blocking => "blocking",
    }
}
