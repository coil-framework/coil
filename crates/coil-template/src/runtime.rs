use super::*;
use std::cmp::Ordering;

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
            &layout.key.name,
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
            &fragment.key.name,
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
        current_template: &TemplateName,
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
                        .get_path(key)
                        .ok_or_else(|| TemplateModelError::MissingValue { key: key.clone() })?;
                    rendered.push_str(&escape_html_text(value.as_text(key)?));
                }
                Node::RawValue(key) => {
                    let value = model
                        .get_path(key)
                        .ok_or_else(|| TemplateModelError::MissingValue { key: key.clone() })?;
                    match value {
                        RenderValue::TrustedHtml(value) => rendered.push_str(value.as_str()),
                        RenderValue::Text(_)
                        | RenderValue::Bool(_)
                        | RenderValue::List(_)
                        | RenderValue::Object(_) => {
                            return Err(TemplateModelError::ValueTypeMismatch {
                                key: key.clone(),
                                expected: "trusted_html",
                            });
                        }
                    }
                }
                Node::Expression(expression) => {
                    let value = self.evaluate_expression(model, expression)?;
                    rendered.push_str(&escape_html_text(&render_expression_as_text(
                        expression, value,
                    )?));
                }
                Node::RawExpression(expression) => {
                    let value = self.evaluate_expression(model, expression)?;
                    match value {
                        RenderValue::TrustedHtml(value) => rendered.push_str(value.as_str()),
                        RenderValue::Text(_)
                        | RenderValue::Bool(_)
                        | RenderValue::List(_)
                        | RenderValue::Object(_) => {
                            return Err(TemplateModelError::ValueTypeMismatch {
                                key: expression_label(expression),
                                expected: "trusted_html",
                            });
                        }
                    }
                }
                Node::Element(element) => {
                    if element.tag == "coil:block" {
                        rendered.push_str(&self.render_nodes(
                            namespaces,
                            current_template,
                            model,
                            slots,
                            &element.children,
                            surface,
                        )?);
                        continue;
                    }
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
                                let value = model.get_path(key).ok_or_else(|| {
                                    TemplateModelError::MissingValue { key: key.clone() }
                                })?;
                                rendered.push_str(&escape_html_attribute(value.as_text(key)?));
                            }
                            AttributeValue::DynamicExpression(expression) => {
                                let value = self.evaluate_expression(model, expression)?;
                                match value {
                                    RenderValue::Text(value) => {
                                        rendered.push_str(&escape_html_attribute(&value));
                                    }
                                    RenderValue::TrustedHtml(value) => {
                                        rendered.push_str(&escape_html_attribute(value.as_str()));
                                    }
                                    RenderValue::Bool(value) => {
                                        rendered
                                            .push_str(&escape_html_attribute(&value.to_string()));
                                    }
                                    RenderValue::List(_) | RenderValue::Object(_) => {
                                        return Err(TemplateModelError::ValueTypeMismatch {
                                            key: attribute.name.clone(),
                                            expected: "text",
                                        });
                                    }
                                }
                            }
                        }
                        rendered.push('"');
                    }
                    rendered.push('>');
                    rendered.push_str(&self.render_nodes(
                        namespaces,
                        current_template,
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
                        rendered.push_str(&self.render_slot_fill(
                            namespaces,
                            current_template,
                            model,
                            fill,
                            surface,
                        )?);
                    } else if let Some(fallback) = &slot.fallback {
                        rendered.push_str(
                            &self.render_nodes(
                                namespaces,
                                current_template,
                                model,
                                slots,
                                fallback,
                                surface,
                            )?,
                        );
                    } else {
                        return Err(TemplateModelError::MissingSlotFill {
                            slot: slot.name.clone(),
                        });
                    }
                }
                Node::With { bindings, children } => {
                    let mut extended = model.clone();
                    for binding in bindings {
                        let value = self.evaluate_expression(model, &binding.expression)?;
                        extended = extended.with_value(binding.key.clone(), value)?;
                    }
                    rendered.push_str(
                        &self.render_nodes(
                            namespaces,
                            current_template,
                            &extended,
                            slots,
                            children,
                            surface,
                        )?,
                    );
                }
                Node::Conditional {
                    condition,
                    negated,
                    children,
                } => {
                    let enabled = self.evaluate_condition(model, condition)?;
                    let enabled = if *negated { !enabled } else { enabled };

                    if enabled {
                        rendered.push_str(
                            &self.render_nodes(
                                namespaces,
                                current_template,
                                model,
                                slots,
                                children,
                                surface,
                            )?,
                        );
                    }
                }
                Node::Switch {
                    expression,
                    cases,
                    default,
                } => {
                    let switch_value = self.evaluate_expression(model, expression)?;
                    let mut matched = false;
                    for case in cases {
                        let case_value = self.evaluate_expression(model, &case.expression)?;
                        if render_values_equal(&switch_value, &case_value, expression_label(expression))? {
                            rendered.push_str(&self.render_nodes(
                                namespaces,
                                current_template,
                                model,
                                slots,
                                &case.children,
                                surface,
                            )?);
                            matched = true;
                            break;
                        }
                    }

                    if !matched {
                        if let Some(default_nodes) = default {
                            rendered.push_str(&self.render_nodes(
                                namespaces,
                                current_template,
                                model,
                                slots,
                                default_nodes,
                                surface,
                            )?);
                        }
                    }
                }
                Node::Case { .. } | Node::Default { .. } => {
                    return Err(TemplateModelError::ParseError {
                        line: 0,
                        column: 0,
                        message: "coil:case and coil:default may only appear inside coil:switch".to_string(),
                    });
                }
                Node::Each {
                    item,
                    collection,
                    children,
                } => {
                    let value = model.get_path(collection).ok_or_else(|| {
                        TemplateModelError::MissingValue {
                            key: collection.clone(),
                        }
                    })?;
                    let entries = value.as_list(collection)?;
                    let known_block_types = collect_block_types(entries);
                    for entry in entries {
                        let dispatch_entry =
                            augment_block_dispatch(entry, current_template, &known_block_types)?;
                        let loop_model = model
                            .merged_with(&dispatch_entry)
                            .with_object(item.clone(), dispatch_entry)?;
                        rendered.push_str(&self.render_nodes(
                            namespaces,
                            current_template,
                            &loop_model,
                            slots,
                            children,
                            surface,
                        )?);
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
                        &template.key.name,
                        model,
                        slots,
                        &template.nodes,
                        surface,
                    )?);
                }
                Node::IncludeExpression(expression) => {
                    let value = self.evaluate_expression(model, expression)?;
                    let name = match value {
                        RenderValue::Text(value) => value,
                        RenderValue::TrustedHtml(value) => value.as_str().to_string(),
                        RenderValue::Bool(_) | RenderValue::List(_) | RenderValue::Object(_) => {
                            return Err(TemplateModelError::ValueTypeMismatch {
                                key: expression_label(expression),
                                expected: "text",
                            });
                        }
                    };
                    let selector = TemplateSelector::new(TemplateName::new(name)?);
                    let template = self.registry.resolve(namespaces, &selector)?;
                    if template.kind != TemplateKind::Fragment {
                        return Err(TemplateModelError::LayoutCannotBeIncludedAsFragment {
                            name: selector.name().clone(),
                        });
                    }
                    rendered.push_str(&self.render_nodes(
                        namespaces,
                        &template.key.name,
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
        current_template: &TemplateName,
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
                    &template.key.name,
                    model,
                    &BTreeMap::new(),
                    &template.nodes,
                    surface,
                )
            }
            SlotFill::Nodes(nodes) => {
                self.render_nodes(
                    namespaces,
                    current_template,
                    model,
                    &BTreeMap::new(),
                    nodes,
                    surface,
                )
            }
        }
    }

    fn evaluate_expression(
        &self,
        model: &RenderModel,
        expression: &TemplateExpression,
    ) -> Result<RenderValue, TemplateModelError> {
        match expression {
            TemplateExpression::ModelKey(key) => model
                .get_path(key)
                .cloned()
                .ok_or_else(|| TemplateModelError::MissingValue { key: key.clone() }),
            TemplateExpression::LiteralText(value) => Ok(RenderValue::text(value.clone())),
            TemplateExpression::LiteralBool(value) => Ok(RenderValue::bool(*value)),
            TemplateExpression::AssetPath(value) => Ok(RenderValue::text(
                model
                    .get_asset_path(value)
                    .unwrap_or(value.as_str())
                    .to_string(),
            )),
            TemplateExpression::TranslationKey(key) => model
                .get_translation(key)
                .map(|value| RenderValue::text(value.to_string()))
                .ok_or_else(|| TemplateModelError::MissingTranslation { key: key.clone() }),
            TemplateExpression::Not(expression) => {
                let value = self.evaluate_expression(model, expression)?;
                Ok(RenderValue::bool(!value.as_bool(&expression_label(expression))?))
            }
            TemplateExpression::Logical {
                left,
                operator,
                right,
            } => {
                let left_value = self.evaluate_expression(model, left)?;
                let left_bool = left_value.as_bool(&expression_label(left))?;
                match operator {
                    LogicalOperator::And if !left_bool => Ok(RenderValue::bool(false)),
                    LogicalOperator::Or if left_bool => Ok(RenderValue::bool(true)),
                    LogicalOperator::And | LogicalOperator::Or => {
                        let right_value = self.evaluate_expression(model, right)?;
                        let right_bool = right_value.as_bool(&expression_label(right))?;
                        Ok(RenderValue::bool(match operator {
                            LogicalOperator::And => left_bool && right_bool,
                            LogicalOperator::Or => left_bool || right_bool,
                        }))
                    }
                }
            }
            TemplateExpression::Compare {
                left,
                operator,
                right,
            } => {
                let left_value = self.evaluate_expression(model, left)?;
                let right_value = self.evaluate_expression(model, right)?;
                Ok(RenderValue::bool(match operator {
                    ComparisonOperator::Equal => render_values_equal(
                        &left_value,
                        &right_value,
                        expression_label(expression),
                    )?,
                    ComparisonOperator::NotEqual => !render_values_equal(
                        &left_value,
                        &right_value,
                        expression_label(expression),
                    )?,
                    ComparisonOperator::GreaterThan => render_values_compare(
                        &left_value,
                        &right_value,
                        expression_label(expression),
                    )? == Ordering::Greater,
                    ComparisonOperator::LessThan => render_values_compare(
                        &left_value,
                        &right_value,
                        expression_label(expression),
                    )? == Ordering::Less,
                    ComparisonOperator::GreaterOrEqual => matches!(
                        render_values_compare(
                            &left_value,
                            &right_value,
                            expression_label(expression),
                        )?,
                        Ordering::Greater | Ordering::Equal
                    ),
                    ComparisonOperator::LessOrEqual => matches!(
                        render_values_compare(
                            &left_value,
                            &right_value,
                            expression_label(expression),
                        )?,
                        Ordering::Less | Ordering::Equal
                    ),
                }))
            }
            TemplateExpression::Elvis { left, right } => {
                if let Some(value) = self.evaluate_elvis_left(model, left)? {
                    Ok(value)
                } else {
                    self.evaluate_expression(model, right)
                }
            }
            TemplateExpression::Conditional {
                condition,
                then_expression,
                else_expression,
            } => {
                let condition_value = self.evaluate_expression(model, condition)?;
                if condition_value.as_bool(&expression_label(condition))? {
                    self.evaluate_expression(model, then_expression)
                } else {
                    self.evaluate_expression(model, else_expression)
                }
            }
        }
    }

    fn evaluate_elvis_left(
        &self,
        model: &RenderModel,
        expression: &TemplateExpression,
    ) -> Result<Option<RenderValue>, TemplateModelError> {
        match self.evaluate_expression(model, expression) {
            Ok(RenderValue::Text(value)) if value.is_empty() => Ok(None),
            Ok(value) => Ok(Some(value)),
            Err(TemplateModelError::MissingValue { .. })
            | Err(TemplateModelError::MissingTranslation { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn evaluate_condition(
        &self,
        model: &RenderModel,
        condition: &ConditionExpression,
    ) -> Result<bool, TemplateModelError> {
        match condition {
            ConditionExpression::Literal(value) => Ok(*value),
            ConditionExpression::Key(key) => {
                let value = model
                    .get_path(key)
                    .ok_or_else(|| TemplateModelError::MissingValue { key: key.clone() })?;
                value.as_bool(key)
            }
            ConditionExpression::Expression(expression) => {
                let value = self.evaluate_expression(model, expression)?;
                value.as_bool(&expression_label(expression))
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

fn render_expression_as_text(
    expression: &TemplateExpression,
    value: RenderValue,
) -> Result<String, TemplateModelError> {
    match value {
        RenderValue::Text(value) => Ok(value),
        RenderValue::TrustedHtml(value) => Ok(value.as_str().to_string()),
        RenderValue::Bool(value) => Ok(value.to_string()),
        RenderValue::List(_) | RenderValue::Object(_) => {
            Err(TemplateModelError::ValueTypeMismatch {
                key: expression_label(expression),
                expected: "text",
            })
        }
    }
}

fn expression_label(expression: &TemplateExpression) -> String {
    match expression {
        TemplateExpression::ModelKey(key) => key.clone(),
        TemplateExpression::LiteralText(value) => value.clone(),
        TemplateExpression::LiteralBool(value) => value.to_string(),
        TemplateExpression::AssetPath(path) => format!("asset({path})"),
        TemplateExpression::TranslationKey(key) => format!("t('{key}')"),
        TemplateExpression::Not(expression) => format!("!{}", expression_label(expression)),
        TemplateExpression::Logical {
            left,
            operator,
            right,
        } => {
            let operator = match operator {
                LogicalOperator::And => "and",
                LogicalOperator::Or => "or",
            };
            format!(
                "{} {operator} {}",
                expression_label(left),
                expression_label(right)
            )
        }
        TemplateExpression::Compare {
            left,
            operator,
            right,
        } => {
            let operator = match operator {
                ComparisonOperator::Equal => "==",
                ComparisonOperator::NotEqual => "!=",
                ComparisonOperator::GreaterThan => ">",
                ComparisonOperator::LessThan => "<",
                ComparisonOperator::GreaterOrEqual => ">=",
                ComparisonOperator::LessOrEqual => "<=",
            };
            format!(
                "{} {operator} {}",
                expression_label(left),
                expression_label(right)
            )
        }
        TemplateExpression::Elvis { left, right } => format!(
            "{} ?: {}",
            expression_label(left),
            expression_label(right)
        ),
        TemplateExpression::Conditional {
            condition,
            then_expression,
            else_expression,
        } => format!(
            "{} ? {} : {}",
            expression_label(condition),
            expression_label(then_expression),
            expression_label(else_expression)
        ),
    }
}

fn render_values_equal(
    left: &RenderValue,
    right: &RenderValue,
    key: String,
) -> Result<bool, TemplateModelError> {
    match (left, right) {
        (RenderValue::Text(left), RenderValue::Text(right)) => Ok(left == right),
        (RenderValue::TrustedHtml(left), RenderValue::TrustedHtml(right)) => {
            Ok(left.as_str() == right.as_str())
        }
        (RenderValue::Bool(left), RenderValue::Bool(right)) => Ok(left == right),
        (RenderValue::List(_), _) | (_, RenderValue::List(_)) => Err(TemplateModelError::ValueTypeMismatch {
            key,
            expected: "scalar",
        }),
        (RenderValue::Object(_), _) | (_, RenderValue::Object(_)) => Err(TemplateModelError::ValueTypeMismatch {
            key,
            expected: "scalar",
        }),
        _ => Ok(false),
    }
}

fn render_values_compare(
    left: &RenderValue,
    right: &RenderValue,
    key: String,
) -> Result<Ordering, TemplateModelError> {
    match (left, right) {
        (RenderValue::Text(left), RenderValue::Text(right)) => Ok(left.cmp(right)),
        (RenderValue::TrustedHtml(left), RenderValue::TrustedHtml(right)) => {
            Ok(left.as_str().cmp(right.as_str()))
        }
        (RenderValue::Bool(left), RenderValue::Bool(right)) => Ok(left.cmp(right)),
        (RenderValue::List(_), _) | (_, RenderValue::List(_)) => Err(TemplateModelError::ValueTypeMismatch {
            key,
            expected: "scalar",
        }),
        (RenderValue::Object(_), _) | (_, RenderValue::Object(_)) => Err(TemplateModelError::ValueTypeMismatch {
            key,
            expected: "scalar",
        }),
        _ => Err(TemplateModelError::ValueTypeMismatch {
            key,
            expected: "comparable_scalar",
        }),
    }
}

pub(crate) fn augment_block_dispatch(
    entry: &RenderModel,
    current_template: &TemplateName,
    known_block_types: &[String],
) -> Result<RenderModel, TemplateModelError> {
    let Some(RenderValue::Text(block_type)) = entry.get_path("type") else {
        return Ok(entry.clone());
    };

    let current_block_type = block_type.clone();
    let dispatch_key = block_dispatch_key(block_type);
    let local_fragment = format!("{}/blocks/{block_type}", current_template.as_str());
    let shared_fragment = format!("blocks/{block_type}");
    let mut model = entry.clone();
    for known_block_type in known_block_types {
        model = model.with_bool(
            format!("is_{}", block_dispatch_key(known_block_type)),
            *known_block_type == current_block_type,
        )?;
    }
    model
        .with_bool(format!("is_{dispatch_key}"), true)?
        .with_value("render_fragment", RenderValue::text(local_fragment))?
        .with_value("render_fragment_shared", RenderValue::text(shared_fragment))
}

fn block_dispatch_key(block_type: &str) -> String {
    let mut key = String::with_capacity(block_type.len());
    for ch in block_type.chars() {
        match ch {
            'a'..='z' | '0'..='9' | '_' => key.push(ch),
            'A'..='Z' => key.push(ch.to_ascii_lowercase()),
            '-' | '.' | ':' | '/' => key.push('_'),
            _ => {}
        }
    }
    if key.is_empty() {
        "block".to_string()
    } else {
        key
    }
}

fn collect_block_types(entries: &[RenderModel]) -> Vec<String> {
    let mut block_types = Vec::new();
    for entry in entries {
        let Some(RenderValue::Text(block_type)) = entry.get_path("type") else {
            continue;
        };
        if !block_types.iter().any(|existing| existing == block_type) {
            block_types.push(block_type.to_string());
        }
    }
    block_types
}
