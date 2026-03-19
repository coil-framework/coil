use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateKind {
    Layout,
    Fragment,
}

impl fmt::Display for TemplateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout => f.write_str("layout"),
            Self::Fragment => f.write_str("fragment"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSelector {
    name: TemplateName,
}

impl TemplateSelector {
    pub fn new(name: TemplateName) -> Self {
        Self { name }
    }

    pub fn name(&self) -> &TemplateName {
        &self.name
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDefinition {
    pub key: TemplateKey,
    pub kind: TemplateKind,
    pub nodes: Vec<Node>,
}

impl TemplateDefinition {
    pub fn layout(namespace: TemplateNamespace, name: TemplateName, nodes: Vec<Node>) -> Self {
        Self {
            key: TemplateKey::new(namespace, name),
            kind: TemplateKind::Layout,
            nodes,
        }
    }

    pub fn fragment(namespace: TemplateNamespace, name: TemplateName, nodes: Vec<Node>) -> Self {
        Self {
            key: TemplateKey::new(namespace, name),
            kind: TemplateKind::Fragment,
            nodes,
        }
    }
}
