use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DefaultSubject {
    Entity(Entity),
    Userset { object: Entity, relation: Relation },
}

impl DefaultSubject {
    pub fn entity(entity: Entity) -> Self {
        Self::Entity(entity)
    }

    pub fn userset(object: Entity, relation: Relation) -> Self {
        Self::Userset { object, relation }
    }

    pub fn to_subject(&self) -> Subject {
        match self {
            Self::Entity(entity) => Subject::Entity(entity.to_object()),
            Self::Userset { object, relation } => Subject::Userset {
                object: object.to_object(),
                relation: relation.to_string(),
            },
        }
    }

    pub fn from_subject(subject: &Subject) -> Option<Self> {
        match subject {
            Subject::Entity(object) => Some(Self::Entity(Entity::from_object(object)?)),
            Subject::Userset { object, relation } => Some(Self::Userset {
                object: Entity::from_object(object)?,
                relation: Relation::from_str(relation)?,
            }),
        }
    }
}

impl From<&DefaultSubject> for Subject {
    fn from(value: &DefaultSubject) -> Self {
        value.to_subject()
    }
}

impl From<DefaultSubject> for Subject {
    fn from(value: DefaultSubject) -> Self {
        value.to_subject()
    }
}
