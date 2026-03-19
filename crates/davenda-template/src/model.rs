use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    DuplicateTemplate {
        key: TemplateKey,
    },
    TemplateNotFound {
        name: TemplateName,
    },
    TemplateKindMismatch {
        name: TemplateName,
        expected: TemplateKind,
        actual: TemplateKind,
    },
    MissingValue {
        key: String,
    },
    MissingSlotFill {
        slot: SlotName,
    },
    FragmentCannotRenderLayout {
        name: TemplateName,
    },
    LayoutCannotBeIncludedAsFragment {
        name: TemplateName,
    },
    InvalidElementName {
        tag: String,
    },
    InvalidAttributeName {
        name: String,
    },
    ValueTypeMismatch {
        key: String,
        expected: &'static str,
    },
}

impl fmt::Display for TemplateModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::DuplicateTemplate { key } => write!(f, "template `{key}` is already registered"),
            Self::TemplateNotFound { name } => write!(f, "template `{name}` was not found"),
            Self::TemplateKindMismatch {
                name,
                expected,
                actual,
            } => write!(
                f,
                "template `{name}` resolved to kind `{actual}` but `{expected}` was required"
            ),
            Self::MissingValue { key } => write!(f, "render value `{key}` was not provided"),
            Self::MissingSlotFill { slot } => write!(f, "slot `{slot}` has no fill or fallback"),
            Self::FragmentCannotRenderLayout { name } => {
                write!(
                    f,
                    "layout template `{name}` cannot be rendered as a fragment"
                )
            }
            Self::LayoutCannotBeIncludedAsFragment { name } => {
                write!(
                    f,
                    "layout template `{name}` cannot be included as a fragment"
                )
            }
            Self::InvalidElementName { tag } => write!(f, "invalid element name `{tag}`"),
            Self::InvalidAttributeName { name } => write!(f, "invalid attribute name `{name}`"),
            Self::ValueTypeMismatch { key, expected } => {
                write!(
                    f,
                    "render value `{key}` does not match expected type `{expected}`"
                )
            }
        }
    }
}

impl Error for TemplateModelError {}

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
pub struct TrustedHtml(String);

impl TrustedHtml {
    pub fn new(value: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self(require_non_empty("trusted_html", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderValue {
    Text(String),
    TrustedHtml(TrustedHtml),
}

impl RenderValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn trusted_html(value: TrustedHtml) -> Self {
        Self::TrustedHtml(value)
    }

    pub(crate) fn as_text(&self, key: &str) -> Result<&str, TemplateModelError> {
        match self {
            Self::Text(value) => Ok(value),
            Self::TrustedHtml(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
        }
    }

    pub(crate) fn render_html(&self) -> String {
        match self {
            Self::Text(value) => escape_html_text(value),
            Self::TrustedHtml(value) => value.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderModel {
    values: BTreeMap<String, RenderValue>,
}

impl RenderModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_value(
        mut self,
        key: impl Into<String>,
        value: RenderValue,
    ) -> Result<Self, TemplateModelError> {
        let key = validate_token("render_key", key.into())?;
        self.values.insert(key, value);
        Ok(self)
    }

    pub(crate) fn get(&self, key: &str) -> Option<&RenderValue> {
        self.values.get(key)
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotFill {
    Template(TemplateSelector),
    Nodes(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentRenderRequest {
    pub(crate) layout: TemplateSelector,
    pub(crate) model: RenderModel,
    pub(crate) slots: BTreeMap<SlotName, SlotFill>,
}

impl DocumentRenderRequest {
    pub fn new(layout: TemplateSelector, model: RenderModel) -> Self {
        Self {
            layout,
            model,
            slots: BTreeMap::new(),
        }
    }

    pub fn with_slot_fill(mut self, slot: SlotName, fill: SlotFill) -> Self {
        self.slots.insert(slot, fill);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentRenderRequest {
    pub(crate) fragment: TemplateSelector,
    pub(crate) model: RenderModel,
}

impl FragmentRenderRequest {
    pub fn new(fragment: TemplateSelector, model: RenderModel) -> Self {
        Self { fragment, model }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub html: String,
}
