use super::*;

use super::index::GraphNode;

pub(super) fn graph_node_from_subject(subject: &Subject) -> GraphNode {
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

pub(super) fn graph_node_from_subject_with_relation(
    subject: &Subject,
    relation: Relation,
) -> GraphNode {
    let object = match subject {
        Subject::Entity(object) | Subject::Userset { object, .. } => object.clone(),
    };

    GraphNode {
        object,
        relation: Some(relation.to_string()),
    }
}

pub(super) fn typed_node(node: &GraphNode) -> Result<ExplainedNode, CoilAuthError> {
    let object = Entity::from_object(&node.object).ok_or_else(|| {
        CoilAuthError::UnsupportedExplainNamespace {
            namespace: node.object.namespace.clone(),
        }
    })?;
    let relation = match &node.relation {
        Some(relation) => Some(parse_relation(relation)?),
        None => None,
    };

    Ok(ExplainedNode { object, relation })
}

pub(super) fn typed_tuple(tuple: &Tuple) -> Result<DefaultTuple, CoilAuthError> {
    DefaultTuple::from_tuple(tuple).ok_or_else(|| {
        if Entity::from_object(&tuple.object).is_none() {
            CoilAuthError::UnsupportedExplainNamespace {
                namespace: tuple.object.namespace.clone(),
            }
        } else {
            CoilAuthError::UnsupportedExplainRelation {
                relation: tuple.relation.clone(),
            }
        }
    })
}

pub(super) fn parse_relation(relation: &str) -> Result<Relation, CoilAuthError> {
    Relation::from_str(relation).ok_or_else(|| CoilAuthError::UnsupportedExplainRelation {
        relation: relation.to_string(),
    })
}
