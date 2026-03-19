use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AccessCheck {
    pub subject: DefaultSubject,
    pub relation: Relation,
    pub object: Entity,
}

impl AccessCheck {
    pub fn new(subject: DefaultSubject, relation: Relation, object: Entity) -> Self {
        Self {
            subject,
            relation,
            object,
        }
    }
}

impl From<AccessCheck> for CheckRequest {
    fn from(value: AccessCheck) -> Self {
        Self {
            subject: value.subject.into(),
            relation: value.relation.to_string(),
            object: value.object.into(),
        }
    }
}
