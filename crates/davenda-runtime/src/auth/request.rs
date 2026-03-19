use davenda_auth::ExplainOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveAuthExplainRequest {
    pub subject: davenda_auth::DefaultSubject,
    pub capability: davenda_auth::Capability,
    pub object: davenda_auth::Entity,
    pub options: ExplainOptions,
}
