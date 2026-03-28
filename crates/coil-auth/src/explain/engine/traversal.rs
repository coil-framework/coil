use super::*;

use super::conversion::{
    graph_node_from_subject, graph_node_from_subject_with_relation, typed_node, typed_tuple,
};
use super::index::{Evaluation, ExplainIndex, GraphNode};
use super::matching::{subject_matches_node, subject_matches_subject};
use super::rules::{inherit_rules_for, userset_jump_rules_for};

pub(super) fn build_relation_trace(
    schema: &Schema,
    tuples: &[Tuple],
    subject: &DefaultSubject,
    relation: Relation,
    object: &Entity,
    options: ExplainOptions,
) -> Result<ExplainTrace, CoilAuthError> {
    let options = options.normalized();
    let root = GraphNode {
        object: object.to_object(),
        relation: Some(relation.to_string()),
    };
    let subject = subject.to_subject();
    let index = ExplainIndex::new(tuples);
    let mut visiting = HashSet::new();

    match explain_node(schema, &index, &subject, &root, 1, options, &mut visiting)? {
        Evaluation::Allowed(mut steps) => {
            steps.insert(
                0,
                ExplainStep::Start {
                    node: typed_node(&root)?,
                },
            );
            Ok(ExplainTrace::Allowed(AllowedExplanation { steps }))
        }
        Evaluation::Denied(denied) => Ok(ExplainTrace::Denied(denied)),
    }
}

fn explain_node(
    schema: &Schema,
    index: &ExplainIndex,
    subject: &Subject,
    node: &GraphNode,
    depth: usize,
    options: ExplainOptions,
    visiting: &mut HashSet<GraphNode>,
) -> Result<Evaluation, CoilAuthError> {
    if options.cycle_protection && !visiting.insert(node.clone()) {
        return Ok(Evaluation::Denied(DeniedExplanation {
            node: typed_node(node)?,
            reason: DeniedReason::CycleDetected,
            attempts: Vec::new(),
        }));
    }

    let result = explain_node_inner(schema, index, subject, node, depth, options, visiting);

    if options.cycle_protection {
        visiting.remove(node);
    }

    result
}

fn explain_node_inner(
    schema: &Schema,
    index: &ExplainIndex,
    subject: &Subject,
    node: &GraphNode,
    depth: usize,
    options: ExplainOptions,
    visiting: &mut HashSet<GraphNode>,
) -> Result<Evaluation, CoilAuthError> {
    if subject_matches_node(node, subject) {
        return Ok(Evaluation::Allowed(vec![ExplainStep::DirectSubjectMatch {
            node: typed_node(node)?,
        }]));
    }

    for tuple in index.tuples_for(node) {
        if subject_matches_subject(&tuple.subject, subject) {
            return Ok(Evaluation::Allowed(vec![ExplainStep::TupleSubjectMatch {
                from: typed_node(node)?,
                tuple: typed_tuple(tuple)?,
            }]));
        }
    }

    if depth >= options.max_depth {
        return Ok(Evaluation::Denied(DeniedExplanation {
            node: typed_node(node)?,
            reason: DeniedReason::RecursionLimitReached {
                max_depth: options.max_depth,
            },
            attempts: Vec::new(),
        }));
    }

    let mut attempts = Vec::new();

    for rule in inherit_rules_for(schema, node)? {
        let next = GraphNode {
            object: node.object.clone(),
            relation: Some(rule.to_string()),
        };
        let step = ExplainStep::Inherit {
            from: typed_node(node)?,
            to: typed_node(&next)?,
        };

        match explain_node(schema, index, subject, &next, depth + 1, options, visiting)? {
            Evaluation::Allowed(mut steps) => {
                steps.insert(0, step);
                return Ok(Evaluation::Allowed(steps));
            }
            Evaluation::Denied(result) => {
                attempts.push(DeniedAttempt::Inherit {
                    step,
                    result: Box::new(result),
                });
            }
        }
    }

    for tuple in index.tuples_for(node) {
        let next = graph_node_from_subject(&tuple.subject);
        let step = ExplainStep::TupleTraversal {
            from: typed_node(node)?,
            tuple: typed_tuple(tuple)?,
            to: typed_node(&next)?,
        };

        match explain_node(schema, index, subject, &next, depth + 1, options, visiting)? {
            Evaluation::Allowed(mut steps) => {
                steps.insert(0, step);
                return Ok(Evaluation::Allowed(steps));
            }
            Evaluation::Denied(result) => {
                attempts.push(DeniedAttempt::TupleTraversal {
                    step,
                    result: Box::new(result),
                });
            }
        }
    }

    for (tuple_relation, target_relation, is_tuple_to_userset) in
        userset_jump_rules_for(schema, node)?
    {
        let jump_node = GraphNode {
            object: node.object.clone(),
            relation: Some(tuple_relation.to_string()),
        };

        for tuple in index.tuples_for(&jump_node) {
            let next = graph_node_from_subject_with_relation(&tuple.subject, target_relation);
            let step = if is_tuple_to_userset {
                ExplainStep::TupleToUserset {
                    from: typed_node(node)?,
                    via_tuple: typed_tuple(tuple)?,
                    to: typed_node(&next)?,
                }
            } else {
                ExplainStep::Computed {
                    from: typed_node(node)?,
                    via_tuple: typed_tuple(tuple)?,
                    to: typed_node(&next)?,
                }
            };

            match explain_node(schema, index, subject, &next, depth + 1, options, visiting)? {
                Evaluation::Allowed(mut steps) => {
                    steps.insert(0, step);
                    return Ok(Evaluation::Allowed(steps));
                }
                Evaluation::Denied(result) => {
                    attempts.push(if is_tuple_to_userset {
                        DeniedAttempt::TupleToUserset {
                            step,
                            result: Box::new(result),
                        }
                    } else {
                        DeniedAttempt::Computed {
                            step,
                            result: Box::new(result),
                        }
                    });
                }
            }
        }
    }

    Ok(Evaluation::Denied(DeniedExplanation {
        node: typed_node(node)?,
        reason: DeniedReason::NoMatchingPath,
        attempts,
    }))
}
