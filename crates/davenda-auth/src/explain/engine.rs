use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GraphNode {
    object: Object,
    relation: Option<String>,
}

#[derive(Debug, Clone)]
struct ExplainIndex {
    tuples_by_node: HashMap<GraphNode, Vec<Tuple>>,
}

impl ExplainIndex {
    fn new(tuples: &[Tuple]) -> Self {
        let mut tuples_by_node = HashMap::new();

        for tuple in tuples {
            let node = GraphNode {
                object: tuple.object.clone(),
                relation: Some(tuple.relation.clone()),
            };
            tuples_by_node
                .entry(node)
                .or_insert_with(Vec::new)
                .push(tuple.clone());
        }

        Self { tuples_by_node }
    }

    fn tuples_for(&self, node: &GraphNode) -> &[Tuple] {
        self.tuples_by_node
            .get(node)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

enum Evaluation {
    Allowed(Vec<ExplainStep>),
    Denied(DeniedExplanation),
}

pub(crate) fn build_capability_explanation<P>(
    package: &P,
    tuples: &[Tuple],
    subject: &DefaultSubject,
    capability: Capability,
    object: &Entity,
    options: ExplainOptions,
) -> Result<CapabilityExplanation, DavendaAuthError>
where
    P: AuthModelPackage,
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

fn build_relation_trace(
    schema: &Schema,
    tuples: &[Tuple],
    subject: &DefaultSubject,
    relation: Relation,
    object: &Entity,
    options: ExplainOptions,
) -> Result<ExplainTrace, DavendaAuthError> {
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
) -> Result<Evaluation, DavendaAuthError> {
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
) -> Result<Evaluation, DavendaAuthError> {
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

fn inherit_rules_for(schema: &Schema, node: &GraphNode) -> Result<Vec<Relation>, DavendaAuthError> {
    let Some(relation) = node.relation.as_deref() else {
        return Ok(Vec::new());
    };

    let Some(config) = schema.namespaces.get(node.object.namespace.as_str()) else {
        return Ok(Vec::new());
    };

    let Some(rules) = config.rules.get(relation) else {
        return Ok(Vec::new());
    };

    let mut parsed = Vec::new();
    for rule in rules {
        if let RelationRule::Inherit(target_relation) = rule {
            parsed.push(parse_relation(target_relation)?);
        }
    }
    Ok(parsed)
}

fn userset_jump_rules_for(
    schema: &Schema,
    node: &GraphNode,
) -> Result<Vec<(Relation, Relation, bool)>, DavendaAuthError> {
    let Some(relation) = node.relation.as_deref() else {
        return Ok(Vec::new());
    };

    let Some(config) = schema.namespaces.get(node.object.namespace.as_str()) else {
        return Ok(Vec::new());
    };

    let Some(rules) = config.rules.get(relation) else {
        return Ok(Vec::new());
    };

    let mut parsed = Vec::new();
    for rule in rules {
        match rule {
            RelationRule::Computed {
                tuple_relation,
                target_relation,
            } => parsed.push((
                parse_relation(tuple_relation)?,
                parse_relation(target_relation)?,
                false,
            )),
            RelationRule::TupleToUserset {
                tuple_relation,
                target_relation,
            } => parsed.push((
                parse_relation(tuple_relation)?,
                parse_relation(target_relation)?,
                true,
            )),
            RelationRule::Inherit(_) => {}
        }
    }
    Ok(parsed)
}

fn graph_node_from_subject(subject: &Subject) -> GraphNode {
    match subject {
        Subject::Entity(object) => GraphNode {
            object: object.clone(),
            relation: None,
        },
        Subject::Userset { object, relation } => GraphNode {
            object: object.clone(),
            relation: Some(relation.clone()),
        },
    }
}

fn graph_node_from_subject_with_relation(subject: &Subject, relation: Relation) -> GraphNode {
    let object = match subject {
        Subject::Entity(object) | Subject::Userset { object, .. } => object.clone(),
    };

    GraphNode {
        object,
        relation: Some(relation.to_string()),
    }
}

fn typed_node(node: &GraphNode) -> Result<ExplainedNode, DavendaAuthError> {
    let object = Entity::from_object(&node.object).ok_or_else(|| {
        DavendaAuthError::UnsupportedExplainNamespace {
            namespace: node.object.namespace.clone(),
        }
    })?;
    let relation = match &node.relation {
        Some(relation) => Some(parse_relation(relation)?),
        None => None,
    };

    Ok(ExplainedNode { object, relation })
}

fn typed_tuple(tuple: &Tuple) -> Result<DefaultTuple, DavendaAuthError> {
    DefaultTuple::from_tuple(tuple).ok_or_else(|| {
        if Entity::from_object(&tuple.object).is_none() {
            DavendaAuthError::UnsupportedExplainNamespace {
                namespace: tuple.object.namespace.clone(),
            }
        } else {
            DavendaAuthError::UnsupportedExplainRelation {
                relation: tuple.relation.clone(),
            }
        }
    })
}

fn parse_relation(relation: &str) -> Result<Relation, DavendaAuthError> {
    Relation::from_str(relation).ok_or_else(|| DavendaAuthError::UnsupportedExplainRelation {
        relation: relation.to_string(),
    })
}

fn subject_matches_node(node: &GraphNode, subject: &Subject) -> bool {
    match subject {
        Subject::Entity(object) => {
            relation_matches(node.relation.as_deref(), None) && object_matches(&node.object, object)
        }
        Subject::Userset { object, relation } => {
            relation_matches(node.relation.as_deref(), Some(relation.as_str()))
                && object_matches(&node.object, object)
        }
    }
}

fn subject_matches_subject(candidate: &Subject, target: &Subject) -> bool {
    match (candidate, target) {
        (Subject::Entity(left), Subject::Entity(right)) => object_matches(left, right),
        (
            Subject::Userset {
                object: left_object,
                relation: left_relation,
            },
            Subject::Userset {
                object: right_object,
                relation: right_relation,
            },
        ) => object_matches(left_object, right_object) && left_relation == right_relation,
        _ => false,
    }
}

fn object_matches(candidate: &Object, target: &Object) -> bool {
    (candidate.namespace == target.namespace || candidate.namespace == "*")
        && (candidate.id == target.id || candidate.id == "*")
}

fn relation_matches(candidate: Option<&str>, target: Option<&str>) -> bool {
    candidate == target
}
