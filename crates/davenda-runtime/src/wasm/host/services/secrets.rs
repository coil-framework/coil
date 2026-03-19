use std::collections::BTreeMap;
use std::sync::Arc;

use davenda_wasm::SecretExecution;

use super::super::*;
use super::keys;

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeSecretBackend {
    values: Arc<BTreeMap<String, String>>,
}

impl RuntimeSecretBackend {
    #[cfg(test)]
    pub(super) fn with_values(values: BTreeMap<String, String>) -> Self {
        Self {
            values: Arc::new(values),
        }
    }

    pub(super) fn read(
        &self,
        secret: &str,
        _context: &InvocationContext,
    ) -> Result<SecretExecution, String> {
        if let Some(value) = self.values.get(secret) {
            return Ok(SecretExecution {
                secret: secret.to_string(),
                source: format!("in-memory:{secret}"),
                value_bytes: value.len(),
            });
        }

        let env_key = secret_env_key(secret);
        let value = std::env::var(&env_key)
            .map_err(|_| format!("secret `{secret}` was not provided to the runtime"))?;

        Ok(SecretExecution {
            secret: secret.to_string(),
            source: format!("env:{env_key}"),
            value_bytes: value.len(),
        })
    }
}

fn secret_env_key(secret: &str) -> String {
    format!("DAVENDA_WASM_SECRET_{}", keys::env_key_component(secret))
}
