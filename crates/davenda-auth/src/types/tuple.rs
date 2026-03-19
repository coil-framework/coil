use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefaultTuple {
    pub object: Entity,
    pub relation: Relation,
    pub subject: DefaultSubject,
}

impl DefaultTuple {
    pub fn new(object: Entity, relation: Relation, subject: DefaultSubject) -> Self {
        Self {
            object,
            relation,
            subject,
        }
    }

    pub fn to_tuple(&self) -> Tuple {
        Tuple {
            object: self.object.to_object(),
            relation: self.relation.to_string(),
            subject: self.subject.to_subject(),
        }
    }

    pub fn from_tuple(tuple: &Tuple) -> Option<Self> {
        Some(Self {
            object: Entity::from_object(&tuple.object)?,
            relation: Relation::from_str(&tuple.relation)?,
            subject: DefaultSubject::from_subject(&tuple.subject)?,
        })
    }
}

impl From<&DefaultTuple> for Tuple {
    fn from(value: &DefaultTuple) -> Self {
        value.to_tuple()
    }
}

impl From<DefaultTuple> for Tuple {
    fn from(value: DefaultTuple) -> Self {
        value.to_tuple()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefaultTupleUpdate {
    Write(DefaultTuple),
    Delete(DefaultTuple),
}

impl From<DefaultTupleUpdate> for TupleUpdate {
    fn from(value: DefaultTupleUpdate) -> Self {
        match value {
            DefaultTupleUpdate::Write(tuple) => TupleUpdate::Write(tuple.into()),
            DefaultTupleUpdate::Delete(tuple) => TupleUpdate::Delete(tuple.into()),
        }
    }
}
