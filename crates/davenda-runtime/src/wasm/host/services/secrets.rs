use std::collections::BTreeMap;
use std::sync::Arc;

use davenda_wasm::SecretExecution;

use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecretBackendMode {
    Runtime,
    Test,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeSecretBackend {
    mode: SecretBackendMode,
    scope: Arc<String>,
    values: Arc<BTreeMap<String, String>>,
}

impl Default for RuntimeSecretBackend {
    fn default() -> Self {
        Self::deny_all("unknown-runtime")
    }
}

impl RuntimeSecretBackend {
    pub(super) fn runtime_scoped(
        scope: impl Into<String>,
        values: BTreeMap<String, String>,
    ) -> Self {
        Self {
            mode: SecretBackendMode::Runtime,
            scope: Arc::new(scope.into()),
            values: Arc::new(values),
        }
    }

    pub(super) fn deny_all(scope: impl Into<String>) -> Self {
        Self {
            mode: SecretBackendMode::Runtime,
            scope: Arc::new(scope.into()),
            values: Arc::new(BTreeMap::new()),
        }
    }

    #[cfg(test)]
    pub(super) fn with_values(values: BTreeMap<String, String>) -> Self {
        Self {
            mode: SecretBackendMode::Test,
            scope: Arc::new("test-runtime".to_string()),
            values: Arc::new(values),
        }
    }

    pub(super) fn read(
        &self,
        secret: &str,
        _context: &InvocationContext,
    ) -> Result<SecretExecution, String> {
        if let Some(value) = self.values.get(secret) {
            let source = match self.mode {
                SecretBackendMode::Runtime => format!("runtime:{}:{secret}", self.scope),
                SecretBackendMode::Test => format!("in-memory:{secret}"),
            };
            return Ok(SecretExecution {
                secret: secret.to_string(),
                source,
                value_bytes: value.len(),
            });
        }

        Err(format!(
            "secret `{secret}` was not provided to runtime `{}`",
            self.scope
        ))
    }
}
