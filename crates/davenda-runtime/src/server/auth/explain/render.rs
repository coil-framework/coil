use davenda_auth::{
    AllowedExplanation, CapabilityExplanation, DeniedAttempt, DeniedExplanation, DeniedReason,
    ExplainDecision, ExplainStep, ExplainTrace,
};
use serde_json::{Value, json};

use davenda_auth::LiveAuthExplainRequest;

pub(crate) fn render_explanation_json(
    tenant_id: i64,
    request: &LiveAuthExplainRequest,
    explanation: &CapabilityExplanation,
) -> Value {
    json!({
        "tenant_id": tenant_id,
        "subject": render_subject(&request.subject),
        "capability": request.capability.as_str(),
        "resource": request.object.to_string(),
        "decision": match explanation.decision {
            ExplainDecision::Allow => "allow",
            ExplainDecision::Deny => "deny",
        },
        "binding": {
            "capability": explanation.binding.capability.as_str(),
            "relation": explanation.binding.relation.as_str(),
            "resource_namespaces": explanation
                .binding
                .resource_namespaces
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        },
        "trace": trace_to_json(&explanation.trace),
    })
}

fn trace_to_json(trace: &ExplainTrace) -> Value {
    match trace {
        ExplainTrace::Allowed(AllowedExplanation { steps }) => json!({
            "kind": "allowed",
            "steps": steps.iter().map(step_to_json).collect::<Vec<_>>(),
        }),
        ExplainTrace::Denied(DeniedExplanation {
            node,
            reason,
            attempts,
        }) => json!({
            "kind": "denied",
            "node": render_node(node),
            "reason": render_reason(reason),
            "attempts": attempts.iter().map(attempt_to_json).collect::<Vec<_>>(),
        }),
    }
}

fn step_to_json(step: &ExplainStep) -> Value {
    match step {
        ExplainStep::Start { node } => json!({
            "kind": "start",
            "node": render_node(node),
        }),
        ExplainStep::DirectSubjectMatch { node } => json!({
            "kind": "direct_subject_match",
            "node": render_node(node),
        }),
        ExplainStep::TupleSubjectMatch { from, tuple } => json!({
            "kind": "tuple_subject_match",
            "from": render_node(from),
            "tuple": render_tuple(tuple),
        }),
        ExplainStep::Inherit { from, to } => json!({
            "kind": "inherit",
            "from": render_node(from),
            "to": render_node(to),
        }),
        ExplainStep::TupleTraversal { from, tuple, to } => json!({
            "kind": "tuple_traversal",
            "from": render_node(from),
            "tuple": render_tuple(tuple),
            "to": render_node(to),
        }),
        ExplainStep::Computed { from, via_tuple, to } => json!({
            "kind": "computed",
            "from": render_node(from),
            "via_tuple": render_tuple(via_tuple),
            "to": render_node(to),
        }),
        ExplainStep::TupleToUserset { from, via_tuple, to } => json!({
            "kind": "tuple_to_userset",
            "from": render_node(from),
            "via_tuple": render_tuple(via_tuple),
            "to": render_node(to),
        }),
    }
}

fn attempt_to_json(attempt: &DeniedAttempt) -> Value {
    match attempt {
        DeniedAttempt::Inherit { step, result } => json!({
            "kind": "inherit",
            "step": step_to_json(step),
            "result": trace_to_json(&ExplainTrace::Denied((**result).clone())),
        }),
        DeniedAttempt::TupleTraversal { step, result } => json!({
            "kind": "tuple_traversal",
            "step": step_to_json(step),
            "result": trace_to_json(&ExplainTrace::Denied((**result).clone())),
        }),
        DeniedAttempt::Computed { step, result } => json!({
            "kind": "computed",
            "step": step_to_json(step),
            "result": trace_to_json(&ExplainTrace::Denied((**result).clone())),
        }),
        DeniedAttempt::TupleToUserset { step, result } => json!({
            "kind": "tuple_to_userset",
            "step": step_to_json(step),
            "result": trace_to_json(&ExplainTrace::Denied((**result).clone())),
        }),
    }
}

fn render_node(node: &davenda_auth::ExplainedNode) -> String {
    match &node.relation {
        Some(relation) => format!("{}#{}", node.object, relation),
        None => node.object.to_string(),
    }
}

fn render_tuple(tuple: &davenda_auth::DefaultTuple) -> String {
    format!(
        "{}#{}={}",
        tuple.object,
        tuple.relation,
        render_subject(&tuple.subject)
    )
}

fn render_subject(subject: &davenda_auth::DefaultSubject) -> String {
    match subject {
        davenda_auth::DefaultSubject::Entity(entity) => entity.to_string(),
        davenda_auth::DefaultSubject::Userset { object, relation } => {
            format!("{}#{}", object, relation)
        }
    }
}

fn render_reason(reason: &DeniedReason) -> String {
    match reason {
        DeniedReason::NoMatchingPath => "no matching path".to_string(),
        DeniedReason::RecursionLimitReached { max_depth } => {
            format!("recursion limit reached at depth {max_depth}")
        }
        DeniedReason::CycleDetected => "cycle detected".to_string(),
    }
}
