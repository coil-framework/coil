#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectStoreClientConfig {
    pub endpoint_url: String,
    pub credential: Option<String>,
}

impl ObjectStoreClientConfig {
    pub fn new(endpoint_url: impl Into<String>) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            credential: None,
        }
    }

    pub fn with_credential(mut self, credential: Option<String>) -> Self {
        self.credential = credential;
        self
    }
}
