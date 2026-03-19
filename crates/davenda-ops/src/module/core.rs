use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsModule {
    name: String,
    config_namespace: String,
}

impl OpsModule {
    pub fn new() -> Self {
        Self {
            name: "ops".to_string(),
            config_namespace: "ops".to_string(),
        }
    }

    pub fn planner(
        &self,
        runtime: JobsRuntime,
        manifests: &[ModuleManifest],
    ) -> Result<OpsPlanner, OpsModelError> {
        OpsPlanner::new(runtime, OpsCatalog::from_manifests(manifests)?)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config_namespace(&self) -> &str {
        &self.config_namespace
    }
}

impl Default for OpsModule {
    fn default() -> Self {
        Self::new()
    }
}
