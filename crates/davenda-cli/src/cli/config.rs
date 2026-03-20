use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValidateInvocation {
    pub config_path: PathBuf,
}
