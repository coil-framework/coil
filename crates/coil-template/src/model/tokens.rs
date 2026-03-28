use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateNamespace(String);

impl TemplateNamespace {
    pub fn new(value: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self(validate_token("template_namespace", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TemplateNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateName(String);

impl TemplateName {
    pub fn new(value: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self(validate_token("template_name", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TemplateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotName(String);

impl SlotName {
    pub fn new(value: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self(validate_token("slot_name", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SlotName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateKey {
    pub namespace: TemplateNamespace,
    pub name: TemplateName,
}

impl TemplateKey {
    pub fn new(namespace: TemplateNamespace, name: TemplateName) -> Self {
        Self { namespace, name }
    }
}

impl fmt::Display for TemplateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}::{}", self.namespace, self.name)
    }
}
