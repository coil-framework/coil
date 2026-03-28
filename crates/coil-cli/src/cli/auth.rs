use crate::cli::args::AuthExplainInvocation;
use coil_auth::CapabilityExplanation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplainResult {
    pub invocation: AuthExplainInvocation,
    pub explanation: CapabilityExplanation,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::AuthExplainInvocation;
    use crate::cli::backend::{AuthExplainBackend, StaticAuthExplainBackend};
    use coil_auth::{
        AllowedExplanation, AuthModelPackage, Capability, DefaultAuthModelPackage, DefaultSubject,
        Entity, ExplainDecision, ExplainOptions, ExplainStep, ExplainTrace,
    };
    use std::path::PathBuf;

    #[test]
    fn execute_auth_explain_with_static_backend_returns_the_provided_explanation() {
        let package = DefaultAuthModelPackage::default();
        let subject = DefaultSubject::entity(Entity::user("alice"));
        let capability = Capability::CmsPageRead;
        let resource = Entity::page("homepage");
        let explanation = coil_auth::CapabilityExplanation {
            manifest: package.manifest().clone(),
            subject: subject.clone(),
            capability,
            object: resource.clone(),
            binding: package.binding_for(capability).unwrap().clone(),
            decision: ExplainDecision::Allow,
            options: ExplainOptions::default(),
            trace: ExplainTrace::Allowed(AllowedExplanation {
                steps: vec![ExplainStep::Start {
                    node: coil_auth::ExplainedNode {
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

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let rendered_explanation = runtime
            .block_on(async { backend.explain(&invocation).await })
            .unwrap();
        let result = AuthExplainResult {
            invocation: invocation.clone(),
            explanation: rendered_explanation,
        };

        assert_eq!(result.invocation, invocation);
        assert_eq!(result.explanation, explanation);
        assert_eq!(backend.requests(), vec![invocation]);
    }
}
