use super::*;

mod conversion;
mod index;
mod matching;
mod rules;
mod traversal;

use traversal::build_relation_trace;

pub(crate) fn build_capability_explanation<P>(
    package: &P,
    tuples: &[Tuple],
    subject: &DefaultSubject,
    capability: Capability,
    object: &Entity,
    options: ExplainOptions,
) -> Result<CapabilityExplanation, DavendaAuthError>
where
    P: AuthModelPackage + ?Sized,
{
    let binding = package.resolve_binding(capability, object)?.clone();
    let trace = build_relation_trace(
        package.schema(),
        tuples,
        subject,
        binding.relation,
        object,
        options,
    )?;
    let decision = match &trace {
        ExplainTrace::Allowed(_) => ExplainDecision::Allow,
        ExplainTrace::Denied(_) => ExplainDecision::Deny,
    };

    Ok(CapabilityExplanation {
        manifest: package.manifest().clone(),
        subject: subject.clone(),
        capability,
        object: object.clone(),
        binding,
        decision,
        options: options.normalized(),
        trace,
    })
}
