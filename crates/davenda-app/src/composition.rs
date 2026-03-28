use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppComposition {
    pub app_id: CustomerAppId,
    pub display_name: String,
    pub domains: Vec<AppDomain>,
    pub sites: Vec<AppSite>,
    pub default_locale: LocaleTag,
    pub supported_locales: Vec<LocaleTag>,
    pub installed_modules: Vec<InstalledModuleSpec>,
    pub module_inventory: Vec<InstalledModuleSummary>,
    pub required_core_services: Vec<CoreServiceDependency>,
    pub migrations: Vec<MigrationContract>,
    pub route_surfaces: Vec<RouteSurface>,
    pub jobs: Vec<JobContract>,
    pub event_subscriptions: Vec<EventSubscription>,
    pub admin_resources: Vec<AdminResourceContribution>,
    pub search_contributions: Vec<SearchIndexContribution>,
    pub report_definitions: Vec<ReportDefinition>,
    pub bulk_operations: Vec<BulkOperationDefinition>,
    pub theme: ThemeProfile,
    pub content_models: Vec<ContentModel>,
    pub extensions: Vec<CustomerExtension>,
    pub auth: AuthStrategy,
}

#[derive(Debug, Clone)]
pub struct CustomerAppRuntimePlan {
    pub composition: CustomerAppComposition,
    pub runtime: RuntimePlan,
    pub theme_publication: Option<davenda_assets::ThemeAssetPublicationReceipt>,
    pub migration_summary: MigrationPlanSummary,
    pub release_doctor: ReleaseDoctorReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledModuleSummary {
    pub id: ModuleId,
    pub version_req: Option<String>,
    pub module_dependencies: Vec<ModuleDependency>,
    pub core_service_dependencies: Vec<CoreServiceDependency>,
    pub migrations: Vec<MigrationContract>,
    pub route_surfaces: Vec<RouteSurface>,
    pub jobs: Vec<JobContract>,
    pub event_subscriptions: Vec<EventSubscription>,
    pub admin_resources: Vec<AdminResourceContribution>,
    pub search_contributions: Vec<SearchIndexContribution>,
    pub report_definitions: Vec<ReportDefinition>,
    pub bulk_operations: Vec<BulkOperationDefinition>,
}

impl CustomerAppComposition {
    pub fn module_list(&self) -> &[InstalledModuleSummary] {
        &self.module_inventory
    }

    pub fn canonical_domain(&self) -> Option<&str> {
        self.primary_site()
            .and_then(|site| site.canonical_domain())
            .or_else(|| {
                self.domains
                    .iter()
                    .find(|domain| domain.canonical)
                    .map(|domain| domain.hostname.as_str())
            })
    }

    pub fn primary_site(&self) -> Option<&AppSite> {
        self.sites.first()
    }

    pub fn site(&self, site_id: &str) -> Option<&AppSite> {
        self.sites.iter().find(|site| site.id.as_str() == site_id)
    }

    pub fn release_doctor(&self, config: Option<&PlatformConfig>) -> ReleaseDoctorReport {
        let mut findings = Vec::new();
        let installed_modules = self
            .installed_modules
            .iter()
            .map(|module| module.id.to_string())
            .collect::<Vec<_>>();

        for module in &self.module_inventory {
            if module.version_req.is_none() {
                findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Warning,
                    "module.version.unpinned",
                    format!(
                        "official module `{}` is not version pinned in the customer app manifest",
                        module.id
                    ),
                ));
            }
        }

        if self.theme.asset_roots.is_empty() {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Warning,
                "theme.assets.missing",
                "the active theme declares no asset roots, so asset publication will be a no-op",
            ));
        }

        if !self.admin_resources.is_empty()
            && !installed_modules.iter().any(|module| module == "admin")
        {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "module.admin.missing",
                "admin resources are composed into the customer app but the `admin` module is not installed",
            ));
        }

        if (!self.search_contributions.is_empty()
            || !self.report_definitions.is_empty()
            || !self.bulk_operations.is_empty())
            && !installed_modules.iter().any(|module| module == "ops")
        {
            findings.push(ReleaseDoctorFinding::new(
                ReleaseDoctorSeverity::Blocking,
                "module.ops.missing",
                "search, reporting, or bulk-operation contracts are present but the `ops` module is not installed",
            ));
        }

        if let Some(config) = config {
            findings.extend(config_alignment_findings(self, config));

            if !config.assets.publish_manifest && !self.theme.asset_roots.is_empty() {
                findings.push(ReleaseDoctorFinding::new(
                    ReleaseDoctorSeverity::Warning,
                    "assets.publish.disabled",
                    "theme assets are declared but `assets.publish_manifest` is disabled in config",
                ));
            }
        }

        ReleaseDoctorReport {
            app_id: self.app_id.clone(),
            findings,
        }
    }

    pub fn module_list_report(&self) -> Result<CommandReport, AppModelError> {
        let mut report = CommandReport::new(
            ["module", "list"],
            format!("Installed modules for customer app `{}`", self.app_id),
        )?
        .with_columns([
            "module",
            "version",
            "core_services",
            "module_dependencies",
            "routes",
            "jobs",
            "admin_resources",
        ])?;

        if self
            .module_inventory
            .iter()
            .any(|module| module.version_req.is_none())
        {
            report = report.with_status(ReportStatus::Warning);
        }

        for module in &self.module_inventory {
            report.push_row(
                ReportRow::new()
                    .with_cell("module", module.id.to_string())?
                    .with_cell(
                        "version",
                        module
                            .version_req
                            .clone()
                            .unwrap_or_else(|| "unpinned".to_string()),
                    )?
                    .with_cell(
                        "core_services",
                        join_display(
                            module
                                .core_service_dependencies
                                .iter()
                                .map(|dependency| format!("{dependency:?}")),
                        ),
                    )?
                    .with_cell(
                        "module_dependencies",
                        if module.module_dependencies.is_empty() {
                            "none".to_string()
                        } else {
                            module
                                .module_dependencies
                                .iter()
                                .map(|dependency| dependency.module.clone())
                                .collect::<Vec<_>>()
                                .join(",")
                        },
                    )?
                    .with_cell("routes", module.route_surfaces.len().to_string())?
                    .with_cell("jobs", module.jobs.len().to_string())?
                    .with_cell("admin_resources", module.admin_resources.len().to_string())?,
            );
        }

        Ok(report)
    }
}

impl From<RuntimeBuildError> for AppModelError {
    fn from(error: RuntimeBuildError) -> Self {
        Self::RuntimeBuild {
            message: error.to_string(),
        }
    }
}
