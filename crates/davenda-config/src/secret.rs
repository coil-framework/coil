use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SecretRef {
    Env { var: String },
    SecretManager { provider: String, key: String },
}

impl SecretRef {
    pub fn redacted(&self) -> String {
        match self {
            Self::Env { var } => format!("env:{var}"),
            Self::SecretManager { provider, key } => format!("secret-manager:{provider}:{key}"),
        }
    }
}
