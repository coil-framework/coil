use crate::cli::auth::AuthExplainResult;
use crate::cli::error::CliRunError;
use crate::command::OutputMode;
use crate::{CommandReport, DiagnosticSeverity, ReportStatus};
use davenda_auth::{
    AllowedExplanation, DeniedAttempt, DeniedExplanation, DeniedReason, ExplainDecision,
    ExplainStep, ExplainTrace,
};
use serde_json::{json, Value};
use std::fmt::Write as _;

pub(crate) fn render_auth_explain(
    result: &AuthExplainResult,
    output_mode: OutputMode,
) -> Result<String, CliRunError> {
    match output_mode {
        OutputMode::Human => Ok(render_human(result)),
        OutputMode::Json => render_json(result),
    }
}

pub(crate) fn render_command_report(
    report: &CommandReport,
    output_mode: OutputMode,
) -> Result<String, CliRunError> {
    match output_mode {
        OutputMode::Human => Ok(render_report_human(report)),
        OutputMode::Json => serde_json::to_string_pretty(report)
            .map_err(|error| CliRunError::execution(format!("failed to encode report JSON: {error}"))),
    }
}

fn render_human(result: &AuthExplainResult) -> String {
    let explanation = &result.explanation;
    let mut out = String::new();
    let _ = writeln!(out, "auth explain");
    let _ = writeln!(out, "  config: {}", result.invocation.config_path.display());
    let _ = writeln!(
        out,
        "  subject: {}",
        render_subject(&result.invocation.subject)
    );
    let _ = writeln!(out, "  capability: {}", result.invocation.capability);
    let _ = writeln!(out, "  resource: {}", result.invocation.resource);
    let _ = writeln!(
        out,
        "  decision: {}",
        match explanation.decision {
            ExplainDecision::Allow => "allow",
            ExplainDecision::Deny => "deny",
        }
    );
    let _ = writeln!(out, "  binding relation: {}", explanation.binding.relation);
    let _ = writeln!(
        out,
        "  binding namespaces: {}",
        explanation
            .binding
            .resource_namespaces
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    );
    let _ = writeln!(out, "  trace:");
    render_trace_human(&mut out, &explanation.trace, 2);
    out
}

fn render_report_human(report: &CommandReport) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", report.command.join(" "));
    let _ = writeln!(out, "  status: {}", render_report_status(report.status));
    let _ = writeln!(out, "  summary: {}", report.summary);

    if !report.columns.is_empty() {
        let _ = writeln!(out, "  columns: {}", report.columns.join(", "));
    }

    for row in &report.rows {
        let _ = writeln!(out, "  row:");
        for column in &report.columns {
            if let Some(value) = row.cells.get(column) {
                let _ = writeln!(out, "    {column}: {value}");
            }
        }
    }

    for diagnostic in &report.diagnostics {
        let _ = writeln!(
            out,
            "  diagnostic [{}] {}: {}",
            render_diagnostic_severity(diagnostic.severity),
            diagnostic.code,
            diagnostic.message
        );
    }

    out.trim_end().to_string()
}

fn render_report_status(status: ReportStatus) -> &'static str {
    match status {
        ReportStatus::Ok => "ok",
        ReportStatus::Warning => "warning",
        ReportStatus::Unsafe => "unsafe",
    }
}

fn render_diagnostic_severity(severity: DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Info => "info",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Error => "error",
    }
}

fn render_trace_human(out: &mut String, trace: &ExplainTrace, indent: usize) {
    let pad = "  ".repeat(indent);
    match trace {
        ExplainTrace::Allowed(AllowedExplanation { steps }) => {
            let _ = writeln!(out, "{pad}allowed steps:");
            for step in steps {
                let _ = writeln!(out, "{pad}  - {}", render_step(step));
            }
        }
        ExplainTrace::Denied(DeniedExplanation {
            node,
            reason,
            attempts,
        }) => {
            let _ = writeln!(
                out,
                "{pad}denied at {} because {}",
                render_node(node),
                render_reason(reason)
            );
            for attempt in attempts {
                render_denied_attempt(out, attempt, indent + 1);
            }
        }
    }
}

fn render_denied_attempt(out: &mut String, attempt: &DeniedAttempt, indent: usize) {
    let pad = "  ".repeat(indent);
    match attempt {
        DeniedAttempt::Inherit { step, result } => {
            let _ = writeln!(out, "{pad}- inherit {}", render_step(step));
            render_trace_human(out, &ExplainTrace::Denied((**result).clone()), indent + 1);
        }
        DeniedAttempt::TupleTraversal { step, result } => {
            let _ = writeln!(out, "{pad}- tuple traversal {}", render_step(step));
            render_trace_human(out, &ExplainTrace::Denied((**result).clone()), indent + 1);
        }
        DeniedAttempt::Computed { step, result } => {
            let _ = writeln!(out, "{pad}- computed {}", render_step(step));
            render_trace_human(out, &ExplainTrace::Denied((**result).clone()), indent + 1);
        }
        DeniedAttempt::TupleToUserset { step, result } => {
            let _ = writeln!(out, "{pad}- tuple-to-userset {}", render_step(step));
            render_trace_human(out, &ExplainTrace::Denied((**result).clone()), indent + 1);
        }
    }
}

fn render_step(step: &ExplainStep) -> String {
    match step {
        ExplainStep::Start { node } => format!("start {}", render_node(node)),
        ExplainStep::DirectSubjectMatch { node } => {
            format!("direct subject match at {}", render_node(node))
        }
        ExplainStep::TupleSubjectMatch { from, tuple } => {
            format!("{} matched {}", render_node(from), render_tuple(tuple))
        }
        ExplainStep::Inherit { from, to } => {
            format!("{} inherits {}", render_node(from), render_node(to))
        }
        ExplainStep::TupleTraversal { from, tuple, to } => format!(
            "{} traverses {} to {}",
            render_node(from),
            render_tuple(tuple),
            render_node(to)
        ),
        ExplainStep::Computed {
            from,
            via_tuple,
            to,
        } => format!(
            "{} computes via {} to {}",
            render_node(from),
            render_tuple(via_tuple),
            render_node(to)
        ),
        ExplainStep::TupleToUserset {
            from,
            via_tuple,
            to,
        } => format!(
            "{} jumps via {} to {}",
            render_node(from),
            render_tuple(via_tuple),
            render_node(to)
        ),
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

fn render_json(result: &AuthExplainResult) -> Result<String, CliRunError> {
    let explanation = &result.explanation;
    let value = json!({
        "command": ["auth", "explain"],
        "config": result.invocation.config_path.display().to_string(),
        "subject": render_subject(&result.invocation.subject),
        "capability": result.invocation.capability.as_str(),
        "resource": result.invocation.resource.to_string(),
        "decision": match explanation.decision {
            ExplainDecision::Allow => "allow",
            ExplainDecision::Deny => "deny",
        },
        "binding": {
            "capability": explanation.binding.capability.as_str(),
            "relation": explanation.binding.relation.as_str(),
            "resource_namespaces": explanation.binding.resource_namespaces.iter().map(ToString::to_string).collect::<Vec<_>>(),
        },
        "trace": trace_to_json(&explanation.trace),
    });

    serde_json::to_string_pretty(&value)
        .map_err(|error| CliRunError::execution(format!("failed to render JSON output: {error}")))
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
        ExplainStep::Computed {
            from,
            via_tuple,
            to,
        } => json!({
            "kind": "computed",
            "from": render_node(from),
            "via_tuple": render_tuple(via_tuple),
            "to": render_node(to),
        }),
        ExplainStep::TupleToUserset {
            from,
            via_tuple,
            to,
        } => json!({
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
