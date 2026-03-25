use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRunInvocation {
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCutoverInvocation {
    pub manifest_path: PathBuf,
    pub apply: bool,
    pub switch: bool,
    pub observe: bool,
    pub rollback: bool,
    pub base_url: Option<String>,
    pub reason: Option<String>,
    pub confirmed: bool,
    pub legacy_freeze_confirmed: bool,
}
