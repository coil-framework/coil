use super::*;

use super::conversion::parse_relation;
use super::index::GraphNode;

pub(super) fn inherit_rules_for(
    schema: &Schema,
    node: &GraphNode,
) -> Result<Vec<Relation>, CoilAuthError> {
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

pub(super) fn userset_jump_rules_for(
    schema: &Schema,
    node: &GraphNode,
) -> Result<Vec<(Relation, Relation, bool)>, CoilAuthError> {
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
