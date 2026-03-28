#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaModule {
    name: String,
    config_namespace: String,
}

impl MediaModule {
    pub fn new() -> Self {
        Self {
            name: "media".to_string(),
            config_namespace: "media".to_string(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn config_namespace(&self) -> &str {
        &self.config_namespace
    }
}

impl Default for MediaModule {
    fn default() -> Self {
        Self::new()
    }
}
