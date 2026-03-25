use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRunInvocation {
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCutoverInvocation {
    pub manifest_path: PathBuf,
    pub dry_run: bool,
    pub apply: bool,
    pub switch: bool,
    pub observe: bool,
    pub rollback: bool,
    pub base_url: Option<String>,
    pub switch_plan_path: Option<PathBuf>,
    pub switch_zone_id: Option<String>,
    pub switch_resource_id: Option<String>,
    pub switch_target: Option<String>,
    pub dns_zone_id: Option<String>,
    pub dns_target: Option<String>,
    pub reason: Option<String>,
    pub confirmed: bool,
    pub legacy_freeze_confirmed: bool,
}
