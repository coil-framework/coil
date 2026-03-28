use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeValue {
    Static(String),
    DynamicText(String),
    DynamicExpression(TemplateExpression),
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
            value: AttributeValue::DynamicExpression(TemplateExpression::ModelKey(validate_token(
                "render_key",
                key.into(),
            )?)),
        })
    }

    pub fn dynamic_expression(
        name: impl Into<String>,
        expression: TemplateExpression,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self {
            name: validate_attribute_name(name.into())?,
            value: AttributeValue::DynamicExpression(expression),
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
pub enum TemplateExpression {
    ModelKey(String),
    LiteralText(String),
    LiteralBool(bool),
    AssetPath(String),
    TranslationKey(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateBinding {
    pub(crate) key: String,
    pub(crate) expression: TemplateExpression,
}

impl TemplateBinding {
    pub fn new(
        key: impl Into<String>,
        expression: TemplateExpression,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self {
            key: validate_token("render_key", key.into())?,
            expression,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionExpression {
    Key(String),
    Literal(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    StaticText(String),
    Value(String),
    RawValue(String),
    Expression(TemplateExpression),
    RawExpression(TemplateExpression),
    Element(ElementNode),
    Slot(SlotNode),
    Include(TemplateSelector),
    With {
        bindings: Vec<TemplateBinding>,
        children: Vec<Node>,
    },
    Conditional {
        condition: ConditionExpression,
        negated: bool,
        children: Vec<Node>,
    },
    Each {
        item: String,
        collection: String,
        children: Vec<Node>,
    },
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

    pub fn expression(expression: TemplateExpression) -> Self {
        Self::Expression(expression)
    }

    pub fn raw_expression(expression: TemplateExpression) -> Self {
        Self::RawExpression(expression)
    }

    pub fn include(selector: TemplateSelector) -> Self {
        Self::Include(selector)
    }

    pub fn with(bindings: Vec<TemplateBinding>, children: Vec<Node>) -> Self {
        Self::With { bindings, children }
    }

    pub fn conditional(
        condition: impl Into<String>,
        children: Vec<Node>,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self::Conditional {
            condition: ConditionExpression::Key(validate_token("render_key", condition.into())?),
            negated: false,
            children,
        })
    }

    pub fn conditional_literal(value: bool, children: Vec<Node>) -> Self {
        Self::Conditional {
            condition: ConditionExpression::Literal(value),
            negated: false,
            children,
        }
    }

    pub fn conditional_not(
        condition: impl Into<String>,
        children: Vec<Node>,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self::Conditional {
            condition: ConditionExpression::Key(validate_token("render_key", condition.into())?),
            negated: true,
            children,
        })
    }

    pub fn each(
        item: impl Into<String>,
        collection: impl Into<String>,
        children: Vec<Node>,
    ) -> Result<Self, TemplateModelError> {
        Ok(Self::Each {
            item: validate_token("render_key", item.into())?,
            collection: validate_token("render_key", collection.into())?,
            children,
        })
    }
}
