use crate::{Capability, DefaultSubject, Entity, ExplainOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveAuthExplainRequest {
    pub subject: DefaultSubject,
    pub capability: Capability,
    pub object: Entity,
    pub options: ExplainOptions,
}
