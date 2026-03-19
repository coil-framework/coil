use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    Static(String),
    DynamicText(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeNode {
    pub(crate) name: String,
    pub(crate) value: AttributeValue,
}

impl AttributeNode {
    pub fn static_value(
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self {
            name: validate_attribute_name(name.into())?,
            value: AttributeValue::Static(require_non_empty("attribute_value", value.into())?),
        })
    }

    pub fn dynamic_text(
        name: impl Into<String>,
        key: impl Into<String>,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self {
            name: validate_attribute_name(name.into())?,
            value: AttributeValue::DynamicText(validate_token("render_key", key.into())?),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementNode {
    pub(crate) tag: String,
    pub(crate) attributes: Vec<AttributeNode>,
    pub(crate) children: Vec<Node>,
}

impl ElementNode {
    pub fn new(tag: impl Into<String>, children: Vec<Node>) -> Result<Self, TemplateModelError> {
        Ok(Self {
            tag: validate_element_name(tag.into())?,
            attributes: Vec::new(),
            children,
        })
    }

    pub fn with_attribute(mut self, attribute: AttributeNode) -> Self {
        self.attributes.push(attribute);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotNode {
    pub(crate) name: SlotName,
    pub(crate) fallback: Option<Vec<Node>>,
}

impl SlotNode {
    pub fn new(name: SlotName) -> Self {
        Self {
            name,
            fallback: None,
        }
    }

    pub fn with_fallback(mut self, fallback: Vec<Node>) -> Self {
        self.fallback = Some(fallback);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    StaticText(String),
    Value(String),
    RawValue(String),
    Element(ElementNode),
    Slot(SlotNode),
    Include(TemplateSelector),
}

impl Node {
    pub fn static_text(value: impl Into<String>) -> Self {
        Self::StaticText(value.into())
    }

    pub fn value(key: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self::Value(validate_token("render_key", key.into())?))
    }

    pub fn raw_value(key: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self::RawValue(validate_token("render_key", key.into())?))
    }

    pub fn include(selector: TemplateSelector) -> Self {
        Self::Include(selector)
    }
}
