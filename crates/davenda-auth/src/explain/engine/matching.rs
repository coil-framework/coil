use super::*;

use super::index::GraphNode;

pub(super) fn subject_matches_node(node: &GraphNode, subject: &Subject) -> bool {
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

pub(super) fn subject_matches_subject(candidate: &Subject, target: &Subject) -> bool {
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
