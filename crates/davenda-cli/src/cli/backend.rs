use async_trait::async_trait;
use davenda_auth::{
    AuthModelPackage, AuthModelPackageSelection, CapabilityExplanation, DefaultAuthModelPackage,
    LiveAuthExplainHost, LiveAuthExplainRequest,
};
use davenda_config::PlatformConfig;
use std::sync::Arc;

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
    explainer: Arc<LiveAuthExplainHost>,
}

impl LiveAuthExplainBackend {
    pub(crate) fn from_config(config: &PlatformConfig) -> Result<Self, CliRunError> {
        let package = resolve_auth_package(config)?;
        let explainer = LiveAuthExplainHost::from_config(config, package).map_err(|error| {
            CliRunError::execution(format!(
                "failed to initialize the live auth explain backend: {error}"
            ))
        })?;

        Ok(Self {
            explainer: Arc::new(explainer),
        })
    }
}

#[async_trait]
impl AuthExplainBackend for LiveAuthExplainBackend {
    async fn explain(
        &self,
        invocation: &AuthExplainInvocation,
    ) -> Result<CapabilityExplanation, CliRunError> {
        let request = LiveAuthExplainRequest {
            subject: invocation.subject.clone(),
            capability: invocation.capability,
            object: invocation.resource.clone(),
            options: invocation.options,
        };

        self.explainer
            .explain_capability(&request)
            .await
            .map_err(|error| {
                CliRunError::execution(format!("failed to build the auth explanation: {error}"))
            })
    }
}

fn resolve_auth_package(
    config: &PlatformConfig,
) -> Result<AuthModelPackageSelection, CliRunError> {
    let package = DefaultAuthModelPackage::default();
    if config.auth.package == package.manifest().name {
        Ok(AuthModelPackageSelection::new(package))
    } else {
        Err(CliRunError::execution(format!(
            "configured auth package `{}` is not registered with the CLI auth explain backend",
            config.auth.package
        )))
    }
}

#[cfg(test)]
use std::sync::Mutex;

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
