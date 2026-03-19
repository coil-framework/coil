use super::*;

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

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, TemplateModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(TemplateModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_token(
    field: &'static str,
    value: String,
) -> Result<String, TemplateModelError> {
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

pub(crate) fn validate_element_name(value: String) -> Result<String, TemplateModelError> {
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

pub(crate) fn validate_attribute_name(value: String) -> Result<String, TemplateModelError> {
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

pub(crate) fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn escape_html_attribute(value: &str) -> String {
    escape_html_text(value)
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
