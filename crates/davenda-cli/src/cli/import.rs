use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRunInvocation {
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCutoverInvocation {
    pub manifest_path: PathBuf,
    pub apply: bool,
    pub confirmed: bool,
    pub legacy_freeze_confirmed: bool,
}
