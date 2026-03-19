use crate::cli::args::AuthExplainInvocation;
use crate::cli::backend::MemoryRebacEngine;
use crate::cli::error::CliRunError;
use davenda_auth::{DavendaAuth, DefaultAuthModelPackage, DefaultTupleUpdate};
use futures::executor::block_on;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthExplainResult {
    pub invocation: AuthExplainInvocation,
    pub explanation: davenda_auth::CapabilityExplanation,
}

pub(crate) fn execute_auth_explain(
    invocation: AuthExplainInvocation,
) -> Result<AuthExplainResult, CliRunError> {
    block_on(async move {
        let engine = MemoryRebacEngine::new();
        let auth = DavendaAuth::new(engine, invocation.tenant_id);

        auth.apply_default_schema().await.map_err(|error| {
            CliRunError::execution(format!("failed to apply the default auth schema: {error}"))
        })?;

        auth.write(
            invocation
                .tuples
                .iter()
                .cloned()
                .map(DefaultTupleUpdate::Write),
        )
        .await
        .map_err(|error| {
            CliRunError::execution(format!("failed to seed the in-memory auth store: {error}"))
        })?;

        let explanation = auth
            .explain_capability_with_options(
                &DefaultAuthModelPackage::default(),
                &invocation.subject,
                invocation.capability,
                &invocation.resource,
                invocation.options,
            )
            .await
            .map_err(|error| {
                CliRunError::execution(format!("failed to build the auth explanation: {error}"))
            })?;

        Ok(AuthExplainResult {
            invocation,
            explanation,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::args::AuthExplainInvocation;
    use davenda_auth::{
        Capability, DefaultSubject, DefaultTuple, Entity, ExplainOptions, Relation,
    };

    #[test]
    fn auth_explain_executes_allow_path_with_seeded_tuples() {
        let invocation = AuthExplainInvocation {
            tenant_id: 1,
            subject: DefaultSubject::entity(Entity::user("alice")),
            capability: Capability::CmsPageRead,
            resource: Entity::page("homepage"),
            tuples: vec![
                DefaultTuple::new(
                    Entity::page("homepage"),
                    Relation::Site,
                    DefaultSubject::entity(Entity::site("main")),
                ),
                DefaultTuple::new(
                    Entity::site("main"),
                    Relation::Viewer,
                    DefaultSubject::entity(Entity::user("alice")),
                ),
            ],
            options: ExplainOptions::default(),
        };

        let result = execute_auth_explain(invocation).unwrap();
        assert!(result.explanation.decision.is_allowed());
    }
}
