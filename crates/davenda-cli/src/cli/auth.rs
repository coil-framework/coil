use crate::cli::args::AuthExplainInvocation;
use crate::cli::backend::{AuthExplainBackend, LiveAuthExplainBackend};
use crate::cli::error::CliRunError;
use davenda_auth::CapabilityExplanation;
use davenda_config::PlatformConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplainResult {
    pub invocation: AuthExplainInvocation,
    pub explanation: CapabilityExplanation,
}

pub(crate) fn execute_live_auth_explain(
    invocation: AuthExplainInvocation,
) -> Result<AuthExplainResult, CliRunError> {
    let config = PlatformConfig::from_file(&invocation.config_path).map_err(|error| {
        CliRunError::execution(format!(
            "failed to load platform config from `{}`: {error}",
            invocation.config_path.display()
        ))
    })?;
    let backend = load_live_auth_explain_backend(&config)?;
    execute_live_auth_explain_with_backend(&backend, invocation)
}

fn load_live_auth_explain_backend(
    config: &PlatformConfig,
) -> Result<LiveAuthExplainBackend, CliRunError> {
    LiveAuthExplainBackend::from_config(config)
}

pub(crate) fn execute_live_auth_explain_with_backend<B: AuthExplainBackend>(
    backend: &B,
    invocation: AuthExplainInvocation,
) -> Result<AuthExplainResult, CliRunError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            CliRunError::execution(format!("failed to start the CLI async runtime: {error}"))
        })?;

    let explanation = runtime.block_on(async { backend.explain(&invocation).await })?;

    Ok(AuthExplainResult {
        invocation,
        explanation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::AuthExplainInvocation;
    use crate::cli::backend::StaticAuthExplainBackend;
    use davenda_auth::{
        AllowedExplanation, AuthModelPackage, Capability, DefaultAuthModelPackage, DefaultSubject,
        Entity, ExplainDecision, ExplainOptions, ExplainStep, ExplainTrace,
    };
    use std::path::PathBuf;

    #[test]
    fn execute_live_auth_explain_with_static_backend_returns_the_provided_explanation() {
        let package = DefaultAuthModelPackage::default();
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let capability = Capability::CmsPageRead;
        let resource = Entity::page("homepage");
        let explanation = davenda_auth::CapabilityExplanation {
            manifest: package.manifest().clone(),
            subject: subject.clone(),
            capability,
            object: resource.clone(),
            binding: package.binding_for(capability).unwrap().clone(),
            decision: ExplainDecision::Allow,
            options: ExplainOptions::default(),
            trace: ExplainTrace::Allowed(AllowedExplanation {
                steps: vec![ExplainStep::Start {
                    node: davenda_auth::ExplainedNode {
                        object: resource.clone(),
                        relation: None,
                    },
                }],
            }),
        };
        let backend = StaticAuthExplainBackend::new(explanation.clone());
        let invocation = AuthExplainInvocation {
            config_path: PathBuf::from("/tmp/platform.toml"),
            subject,
            capability,
            resource,
            options: ExplainOptions::default(),
        };

        let result = execute_live_auth_explain_with_backend(&backend, invocation.clone()).unwrap();

        assert_eq!(result.invocation, invocation);
        assert_eq!(result.explanation, explanation);
        assert_eq!(backend.requests(), vec![invocation]);
    }
}
