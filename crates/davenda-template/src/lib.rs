use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

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

    fn as_text(&self, key: &str) -> Result<&str, TemplateModelError> {
        match self {
            Self::Text(value) => Ok(value),
            Self::TrustedHtml(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
        }
    }

    fn render_html(&self) -> String {
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

    fn get(&self, key: &str) -> Option<&RenderValue> {
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
    name: String,
    value: AttributeValue,
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
    tag: String,
    attributes: Vec<AttributeNode>,
    children: Vec<Node>,
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
    name: SlotName,
    fallback: Option<Vec<Node>>,
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
    layout: TemplateSelector,
    model: RenderModel,
    slots: BTreeMap<SlotName, SlotFill>,
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
    fragment: TemplateSelector,
    model: RenderModel,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateRegistry {
    templates: BTreeMap<TemplateKey, TemplateDefinition>,
}

impl TemplateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, template: TemplateDefinition) -> Result<(), TemplateModelError> {
        if self.templates.contains_key(&template.key) {
            return Err(TemplateModelError::DuplicateTemplate {
                key: template.key.clone(),
            });
        }

        self.templates.insert(template.key.clone(), template);
        Ok(())
    }

    fn resolve(
        &self,
        namespaces: &[TemplateNamespace],
        selector: &TemplateSelector,
    ) -> Result<&TemplateDefinition, TemplateModelError> {
        for namespace in namespaces {
            let key = TemplateKey::new(namespace.clone(), selector.name().clone());
            if let Some(template) = self.templates.get(&key) {
                return Ok(template);
            }
        }

        Err(TemplateModelError::TemplateNotFound {
            name: selector.name().clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateRuntime {
    registry: TemplateRegistry,
}

impl TemplateRuntime {
    pub fn new(registry: TemplateRegistry) -> Self {
        Self { registry }
    }

    pub fn render_document(
        &self,
        namespaces: &[TemplateNamespace],
        request: DocumentRenderRequest,
    ) -> Result<RenderOutput, TemplateModelError> {
        let layout = self.registry.resolve(namespaces, &request.layout)?;
        if layout.kind != TemplateKind::Layout {
            return Err(TemplateModelError::TemplateKindMismatch {
                name: request.layout.name().clone(),
                expected: TemplateKind::Layout,
                actual: layout.kind,
            });
        }

        let html = self.render_nodes(
            namespaces,
            &request.model,
            &request.slots,
            &layout.nodes,
            RenderSurface::Document,
        )?;

        Ok(RenderOutput { html })
    }

    pub fn render_fragment(
        &self,
        namespaces: &[TemplateNamespace],
        request: FragmentRenderRequest,
    ) -> Result<RenderOutput, TemplateModelError> {
        let fragment = self.registry.resolve(namespaces, &request.fragment)?;
        if fragment.kind != TemplateKind::Fragment {
            return Err(TemplateModelError::FragmentCannotRenderLayout {
                name: request.fragment.name().clone(),
            });
        }

        let html = self.render_nodes(
            namespaces,
            &request.model,
            &BTreeMap::new(),
            &fragment.nodes,
            RenderSurface::Fragment,
        )?;

        Ok(RenderOutput { html })
    }

    fn render_nodes(
        &self,
        namespaces: &[TemplateNamespace],
        model: &RenderModel,
        slots: &BTreeMap<SlotName, SlotFill>,
        nodes: &[Node],
        surface: RenderSurface,
    ) -> Result<String, TemplateModelError> {
        let mut rendered = String::new();
        for node in nodes {
            match node {
                Node::StaticText(value) => rendered.push_str(value),
                Node::Value(key) => {
                    let value = model
                        .get(key)
                        .ok_or_else(|| TemplateModelError::MissingValue { key: key.clone() })?;
                    rendered.push_str(&value.render_html());
                }
                Node::RawValue(key) => {
                    let value = model
                        .get(key)
                        .ok_or_else(|| TemplateModelError::MissingValue { key: key.clone() })?;
                    match value {
                        RenderValue::TrustedHtml(value) => rendered.push_str(value.as_str()),
                        RenderValue::Text(_) => {
                            return Err(TemplateModelError::ValueTypeMismatch {
                                key: key.clone(),
                                expected: "trusted_html",
                            });
                        }
                    }
                }
                Node::Element(element) => {
                    rendered.push('<');
                    rendered.push_str(&element.tag);
                    for attribute in &element.attributes {
                        rendered.push(' ');
                        rendered.push_str(&attribute.name);
                        rendered.push_str("=\"");
                        match &attribute.value {
                            AttributeValue::Static(value) => {
                                rendered.push_str(&escape_html_attribute(value))
                            }
                            AttributeValue::DynamicText(key) => {
                                let value = model.get(key).ok_or_else(|| {
                                    TemplateModelError::MissingValue { key: key.clone() }
                                })?;
                                rendered.push_str(&escape_html_attribute(value.as_text(key)?));
                            }
                        }
                        rendered.push('"');
                    }
                    rendered.push('>');
                    rendered.push_str(&self.render_nodes(
                        namespaces,
                        model,
                        slots,
                        &element.children,
                        surface,
                    )?);
                    rendered.push_str("</");
                    rendered.push_str(&element.tag);
                    rendered.push('>');
                }
                Node::Slot(slot) => {
                    if let Some(fill) = slots.get(&slot.name) {
                        rendered
                            .push_str(&self.render_slot_fill(namespaces, model, fill, surface)?);
                    } else if let Some(fallback) = &slot.fallback {
                        rendered.push_str(
                            &self.render_nodes(namespaces, model, slots, fallback, surface)?,
                        );
                    } else {
                        return Err(TemplateModelError::MissingSlotFill {
                            slot: slot.name.clone(),
                        });
                    }
                }
                Node::Include(selector) => {
                    let template = self.registry.resolve(namespaces, selector)?;
                    if template.kind != TemplateKind::Fragment {
                        return Err(TemplateModelError::LayoutCannotBeIncludedAsFragment {
                            name: selector.name().clone(),
                        });
                    }
                    rendered.push_str(&self.render_nodes(
                        namespaces,
                        model,
                        slots,
                        &template.nodes,
                        surface,
                    )?);
                }
            }
        }

        if surface == RenderSurface::Fragment && rendered.starts_with("<!DOCTYPE") {
            return Err(TemplateModelError::FragmentCannotRenderLayout {
                name: TemplateName::new("document").expect("constant token is valid"),
            });
        }

        Ok(rendered)
    }

    fn render_slot_fill(
        &self,
        namespaces: &[TemplateNamespace],
        model: &RenderModel,
        fill: &SlotFill,
        surface: RenderSurface,
    ) -> Result<String, TemplateModelError> {
        match fill {
            SlotFill::Template(selector) => {
                let template = self.registry.resolve(namespaces, selector)?;
                if template.kind != TemplateKind::Fragment {
                    return Err(TemplateModelError::LayoutCannotBeIncludedAsFragment {
                        name: selector.name().clone(),
                    });
                }
                self.render_nodes(
                    namespaces,
                    model,
                    &BTreeMap::new(),
                    &template.nodes,
                    surface,
                )
            }
            SlotFill::Nodes(nodes) => {
                self.render_nodes(namespaces, model, &BTreeMap::new(), nodes, surface)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderSurface {
    Document,
    Fragment,
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, TemplateModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(TemplateModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, TemplateModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(TemplateModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

fn validate_element_name(value: String) -> Result<String, TemplateModelError> {
    let tag = require_non_empty("element_tag", value)?;
    if tag
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':'))
    {
        Ok(tag)
    } else {
        Err(TemplateModelError::InvalidElementName { tag })
    }
}

fn validate_attribute_name(value: String) -> Result<String, TemplateModelError> {
    let name = require_non_empty("attribute_name", value)?;
    if name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | ':' | '_'))
    {
        Ok(name)
    } else {
        Err(TemplateModelError::InvalidAttributeName { name })
    }
}

fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_html_attribute(value: &str) -> String {
    escape_html_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn namespaces() -> Vec<TemplateNamespace> {
        vec![
            TemplateNamespace::new("customer-app").unwrap(),
            TemplateNamespace::new("events").unwrap(),
            TemplateNamespace::new("core").unwrap(),
        ]
    }

    fn selector(name: &str) -> TemplateSelector {
        TemplateSelector::new(TemplateName::new(name).unwrap())
    }

    fn model() -> RenderModel {
        RenderModel::new()
            .with_value("title", RenderValue::text("Event <Launch>"))
            .unwrap()
            .with_value("headline", RenderValue::text("Book & Save"))
            .unwrap()
            .with_value("cta_class", RenderValue::text("primary\" onclick=\"oops"))
            .unwrap()
            .with_value(
                "trusted_badge",
                RenderValue::trusted_html(
                    TrustedHtml::new("<strong class=\"badge\">Live</strong>").unwrap(),
                ),
            )
            .unwrap()
    }

    fn base_registry() -> TemplateRegistry {
        let mut registry = TemplateRegistry::new();

        registry
            .register(TemplateDefinition::layout(
                TemplateNamespace::new("core").unwrap(),
                TemplateName::new("storefront.layout").unwrap(),
                vec![
                    Node::static_text("<!DOCTYPE html>"),
                    Node::Element(
                        ElementNode::new(
                            "html",
                            vec![Node::Element(
                                ElementNode::new(
                                    "body",
                                    vec![
                                        Node::Slot(
                                            SlotNode::new(SlotName::new("hero").unwrap())
                                                .with_fallback(vec![Node::static_text(
                                                    "<div class=\"hero-fallback\"></div>",
                                                )]),
                                        ),
                                        Node::Element(
                                            ElementNode::new(
                                                "main",
                                                vec![Node::Slot(SlotNode::new(
                                                    SlotName::new("content").unwrap(),
                                                ))],
                                            )
                                            .unwrap(),
                                        ),
                                    ],
                                )
                                .unwrap(),
                            )],
                        )
                        .unwrap(),
                    ),
                ],
            ))
            .unwrap();

        registry
            .register(TemplateDefinition::fragment(
                TemplateNamespace::new("events").unwrap(),
                TemplateName::new("hero").unwrap(),
                vec![Node::Element(
                    ElementNode::new(
                        "section",
                        vec![
                            Node::Element(
                                ElementNode::new("h1", vec![Node::value("headline").unwrap()])
                                    .unwrap(),
                            ),
                            Node::raw_value("trusted_badge").unwrap(),
                        ],
                    )
                    .unwrap()
                    .with_attribute(AttributeNode::static_value("class", "hero").unwrap()),
                )],
            ))
            .unwrap();

        registry
            .register(TemplateDefinition::fragment(
                TemplateNamespace::new("events").unwrap(),
                TemplateName::new("booking.panel").unwrap(),
                vec![Node::Element(
                    ElementNode::new("div", vec![Node::value("title").unwrap()])
                        .unwrap()
                        .with_attribute(
                            AttributeNode::static_value("data-fragment", "booking").unwrap(),
                        )
                        .with_attribute(AttributeNode::dynamic_text("class", "cta_class").unwrap()),
                )],
            ))
            .unwrap();

        registry
            .register(TemplateDefinition::fragment(
                TemplateNamespace::new("customer-app").unwrap(),
                TemplateName::new("hero").unwrap(),
                vec![Node::Element(
                    ElementNode::new(
                        "section",
                        vec![Node::Element(
                            ElementNode::new("h1", vec![Node::static_text("Branded Hero")])
                                .unwrap(),
                        )],
                    )
                    .unwrap()
                    .with_attribute(AttributeNode::static_value("class", "hero customer").unwrap()),
                )],
            ))
            .unwrap();

        registry
    }

    #[test]
    fn document_rendering_composes_layout_slots_and_fragments() {
        let runtime = TemplateRuntime::new(base_registry());
        let output = runtime
            .render_document(
                &namespaces(),
                DocumentRenderRequest::new(selector("storefront.layout"), model())
                    .with_slot_fill(
                        SlotName::new("hero").unwrap(),
                        SlotFill::Template(selector("hero")),
                    )
                    .with_slot_fill(
                        SlotName::new("content").unwrap(),
                        SlotFill::Template(selector("booking.panel")),
                    ),
            )
            .unwrap();

        assert!(output.html.starts_with("<!DOCTYPE html><html><body>"));
        assert!(
            output
                .html
                .contains("<section class=\"hero customer\"><h1>Branded Hero</h1></section>")
        );
        assert!(output.html.contains("<main><div data-fragment=\"booking\" class=\"primary&quot; onclick=&quot;oops\">Event &lt;Launch&gt;</div></main>"));
    }

    #[test]
    fn fragment_rendering_reuses_same_fragment_for_partial_output() {
        let runtime = TemplateRuntime::new(base_registry());
        let output = runtime
            .render_fragment(
                &namespaces(),
                FragmentRenderRequest::new(selector("booking.panel"), model()),
            )
            .unwrap();

        assert_eq!(
            output.html,
            "<div data-fragment=\"booking\" class=\"primary&quot; onclick=&quot;oops\">Event &lt;Launch&gt;</div>"
        );
    }

    #[test]
    fn dynamic_values_escape_html_by_default_and_trusted_html_is_explicit() {
        let runtime = TemplateRuntime::new(base_registry());
        let output = runtime
            .render_fragment(
                &namespaces(),
                FragmentRenderRequest::new(selector("hero"), model()),
            )
            .unwrap();

        assert!(output.html.contains("Branded Hero"));
        let event_output = runtime
            .render_fragment(
                &[TemplateNamespace::new("events").unwrap()],
                FragmentRenderRequest::new(selector("hero"), model()),
            )
            .unwrap();
        assert!(event_output.html.contains("Book &amp; Save"));
        assert!(
            event_output
                .html
                .contains("<strong class=\"badge\">Live</strong>")
        );
    }

    #[test]
    fn namespace_resolution_prefers_customer_app_over_module_templates() {
        let runtime = TemplateRuntime::new(base_registry());

        let output = runtime
            .render_fragment(
                &namespaces(),
                FragmentRenderRequest::new(selector("hero"), model()),
            )
            .unwrap();

        assert!(output.html.contains("hero customer"));
        assert!(!output.html.contains("Book &amp; Save"));
    }

    #[test]
    fn layouts_cannot_be_rendered_as_fragments() {
        let runtime = TemplateRuntime::new(base_registry());

        assert_eq!(
            runtime
                .render_fragment(
                    &namespaces(),
                    FragmentRenderRequest::new(selector("storefront.layout"), model()),
                )
                .unwrap_err(),
            TemplateModelError::FragmentCannotRenderLayout {
                name: TemplateName::new("storefront.layout").unwrap(),
            }
        );
    }

    #[test]
    fn missing_slot_without_fallback_is_rejected() {
        let mut registry = TemplateRegistry::new();
        registry
            .register(TemplateDefinition::layout(
                TemplateNamespace::new("core").unwrap(),
                TemplateName::new("minimal.layout").unwrap(),
                vec![Node::Slot(SlotNode::new(SlotName::new("content").unwrap()))],
            ))
            .unwrap();
        let runtime = TemplateRuntime::new(registry);

        assert_eq!(
            runtime
                .render_document(
                    &[TemplateNamespace::new("core").unwrap()],
                    DocumentRenderRequest::new(selector("minimal.layout"), RenderModel::new()),
                )
                .unwrap_err(),
            TemplateModelError::MissingSlotFill {
                slot: SlotName::new("content").unwrap(),
            }
        );
    }
}
