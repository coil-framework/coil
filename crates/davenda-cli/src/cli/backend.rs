use async_trait::async_trait;
use davenda_auth::{AuthModelPackage, CapabilityExplanation, DavendaAuth, DefaultAuthModelPackage};
use davenda_config::PlatformConfig;
use davenda_data::DataRuntime;

use crate::cli::args::AuthExplainInvocation;
use crate::cli::error::CliRunError;

#[async_trait]
pub(crate) trait AuthExplainBackend: Send + Sync {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError>;
}

#[derive(Debug, Clone)]
pub(crate) struct LiveAuthExplainBackend {
    tenant_id: i64,
    data: DataRuntime,
    package: DefaultAuthModelPackage,
}

impl LiveAuthExplainBackend {
    pub(crate) fn from_config(config: &PlatformConfig) -> Result<Self, CliRunError> {
        if !config.auth.explain_api {
            return Err(CliRunError::execution(
                "auth explain API is disabled by deployment config",
            ));
        }

        let package = DefaultAuthModelPackage::default();
        if config.auth.package != package.manifest().name {
            return Err(CliRunError::execution(format!(
                "configured auth package `{}` is not supported by the CLI explain command; expected `{}`",
                config.auth.package,
                package.manifest().name
            )));
        }

        let data = DataRuntime::from_config(&config.database).map_err(|error| {
            CliRunError::execution(format!(
                "failed to initialize the database runtime for auth explain: {error}"
            ))
        })?;

        Ok(Self {
            tenant_id: config.auth.tenant_id,
            data,
            package,
        })
    }
}

#[async_trait]
impl AuthExplainBackend for LiveAuthExplainBackend {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError> {
        let client = self.data.connect_lazy_postgres().map_err(|error| {
            CliRunError::execution(format!(
                "failed to prepare the PostgreSQL auth backend for explain: {error}"
            ))
        })?;
        let engine = zanzibar::postgres::PostgresRebacEngine::new(client.pool.clone());
        let auth = DavendaAuth::new(engine, self.tenant_id);

        auth.explain_capability_with_options(
            &self.package,
            &invocation.subject,
            invocation.capability,
            &invocation.resource,
            invocation.options,
        )
        .await
        .map_err(|error| {
            CliRunError::execution(format!("failed to build the auth explanation: {error}"))
        })
    }
}

#[cfg(test)]
use std::sync::{Arc, Mutex};

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct StaticAuthExplainBackend {
    response: CapabilityExplanation,
    requests: Arc<Mutex<Vec<AuthExplainInvocation>>>,
}

#[cfg(test)]
impl StaticAuthExplainBackend {
    pub(crate) fn new(response: CapabilityExplanation) -> Self {
        Self {
            response,
            requests: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn requests(&self) -> Vec<AuthExplainInvocation> {
        self.requests
            .lock()
            .expect("static auth explain backend mutex poisoned")
            .clone()
    }
}

#[cfg(test)]
#[async_trait]
impl AuthExplainBackend for StaticAuthExplainBackend {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError> {
        self.requests
            .lock()
            .expect("static auth explain backend mutex poisoned")
            .push(invocation.clone());
        Ok(self.response.clone())
    }
}
