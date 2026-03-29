use super::*;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy)]
pub struct TemplateSourceParser;

impl TemplateSourceParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_layout(
        &self,
        namespace: TemplateNamespace,
        name: TemplateName,
        source: &str,
    ) -> Result<TemplateDefinition, TemplateModelError> {
        self.parse_definition(namespace, name, source, TemplateKind::Layout)
    }

    pub fn parse_fragment(
        &self,
        namespace: TemplateNamespace,
        name: TemplateName,
        source: &str,
    ) -> Result<TemplateDefinition, TemplateModelError> {
        self.parse_definition(namespace, name, source, TemplateKind::Fragment)
    }

    pub fn parse_auto(
        &self,
        namespace: TemplateNamespace,
        name: TemplateName,
        source: &str,
    ) -> Result<TemplateDefinition, TemplateModelError> {
        let kind = if source.contains("coil:fragment=") {
            TemplateKind::Fragment
        } else {
            TemplateKind::Layout
        };
        self.parse_definition(namespace, name, source, kind)
    }

    pub fn load_directory<P>(
        &self,
        root: P,
        namespace: TemplateNamespace,
    ) -> Result<Vec<TemplateDefinition>, TemplateModelError>
    where
        P: AsRef<Path>,
    {
        let root = root.as_ref();
        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut files = Vec::new();
        collect_template_files(root, &mut files)?;
        files.sort();

        let mut templates = Vec::with_capacity(files.len());
        for path in files {
            templates.push(self.parse_file(root, path, namespace.clone())?);
        }

        Ok(templates)
    }

    pub fn parse_file<R, P>(
        &self,
        root: R,
        path: P,
        namespace: TemplateNamespace,
    ) -> Result<TemplateDefinition, TemplateModelError>
    where
        R: AsRef<Path>,
        P: AsRef<Path>,
    {
        let root = root.as_ref();
        let path = path.as_ref();
        let source =
            fs::read_to_string(path).map_err(|error| TemplateModelError::TemplateRead {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;

        let kind = template_kind_for_path(root, path);
        self.parse_source(root, path, &source, namespace, kind)
    }

    pub fn parse_source<R, P>(
        &self,
        root: R,
        path: P,
        source: &str,
        namespace: TemplateNamespace,
        kind: TemplateKind,
    ) -> Result<TemplateDefinition, TemplateModelError>
    where
        R: AsRef<Path>,
        P: AsRef<Path>,
    {
        let root = root.as_ref();
        let path = path.as_ref();
        let relative = path.strip_prefix(root).unwrap_or(path).with_extension("");
        let name = TemplateName::new(relative.to_string_lossy().replace('\\', "/"))?;
        let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(source);

        let nodes = match kind {
            TemplateKind::Layout => render_document_nodes(&dom, path)?,
            TemplateKind::Fragment => render_fragment_nodes(&dom, path)?,
        };

        Ok(match kind {
            TemplateKind::Layout => TemplateDefinition::layout(namespace, name, nodes),
            TemplateKind::Fragment => TemplateDefinition::fragment(namespace, name, nodes),
        })
    }

    fn parse_definition(
        &self,
        namespace: TemplateNamespace,
        name: TemplateName,
        source: &str,
        kind: TemplateKind,
    ) -> Result<TemplateDefinition, TemplateModelError> {
        let dom = html5ever::parse_document(RcDom::default(), Default::default()).one(source);
        let nodes = match kind {
            TemplateKind::Layout => render_document_nodes(&dom, Path::new("<template>"))?,
            TemplateKind::Fragment => render_fragment_nodes(&dom, Path::new("<template>"))?,
        };

        Ok(match kind {
            TemplateKind::Layout => TemplateDefinition::layout(namespace, name, nodes),
            TemplateKind::Fragment => TemplateDefinition::fragment(namespace, name, nodes),
        })
    }
}

fn collect_template_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), TemplateModelError> {
    for entry in fs::read_dir(dir).map_err(|error| TemplateModelError::TemplateRead {
        path: dir.display().to_string(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| TemplateModelError::TemplateRead {
            path: dir.display().to_string(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_template_files(&path, files)?;
            continue;
        }

        if path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("html"))
            .unwrap_or(false)
        {
            files.push(path);
        }
    }

    Ok(())
}

fn template_kind_for_path(root: &Path, path: &Path) -> TemplateKind {
    let relative = path.strip_prefix(root).unwrap_or(path);
    match relative
        .components()
        .next()
        .and_then(|component| component.as_os_str().to_str())
    {
        Some("components") | Some("fragments") => TemplateKind::Fragment,
        _ => TemplateKind::Layout,
    }
}

fn render_document_nodes(dom: &RcDom, path: &Path) -> Result<Vec<Node>, TemplateModelError> {
    let mut rendered = Vec::new();
    for child in dom.document.children.borrow().iter() {
        rendered.extend(render_node(child, path)?);
    }
    Ok(rendered)
}

fn render_fragment_nodes(dom: &RcDom, path: &Path) -> Result<Vec<Node>, TemplateModelError> {
    if let Some(body) = find_body(dom.document.clone()) {
        let mut rendered = Vec::new();
        for child in body.children.borrow().iter() {
            rendered.extend(render_node(child, path)?);
        }
        return Ok(rendered);
    }

    render_document_nodes(dom, path)
}

fn find_body(handle: Handle) -> Option<Handle> {
    for child in handle.children.borrow().iter() {
        if let NodeData::Element { name, .. } = &child.data {
            if name.local.as_ref().eq_ignore_ascii_case("html") {
                for grandchild in child.children.borrow().iter() {
                    if let NodeData::Element { name, .. } = &grandchild.data {
                        if name.local.as_ref().eq_ignore_ascii_case("body") {
                            return Some(grandchild.clone());
                        }
                    }
                }
            }
        }

        if let Some(body) = find_body(child.clone()) {
            return Some(body);
        }
    }

    None
}

fn render_node(handle: &Handle, path: &Path) -> Result<Vec<Node>, TemplateModelError> {
    match &handle.data {
        NodeData::Document => {
            let mut rendered = Vec::new();
            for child in handle.children.borrow().iter() {
                rendered.extend(render_node(child, path)?);
            }
            Ok(rendered)
        }
        NodeData::Doctype { name, .. } => Ok(vec![Node::static_text(format!("<!DOCTYPE {name}>"))]),
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            if text.trim().is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![Node::static_text(text)])
        }
        NodeData::Comment { .. } => Ok(Vec::new()),
        NodeData::Element { name, attrs, .. } => render_element(
            name.local.as_ref(),
            attrs.borrow().iter(),
            handle.children.borrow().iter(),
            path,
        ),
        _ => Ok(Vec::new()),
    }
}

fn render_element<'a>(
    tag: &str,
    attrs: impl Iterator<Item = &'a markup5ever::Attribute>,
    children: impl Iterator<Item = &'a Handle>,
    path: &Path,
) -> Result<Vec<Node>, TemplateModelError> {
    let mut static_attrs = Vec::new();
    let mut dynamic_attrs = Vec::new();
    let mut include_selector: Option<IncludeTarget> = None;
    let mut include_expression: Option<TemplateExpression> = None;
    let mut slot_name: Option<String> = None;
    let mut text_expression: Option<TemplateExpression> = None;
    let mut raw_text_expression: Option<TemplateExpression> = None;
    let mut with_bindings: Vec<TemplateBinding> = Vec::new();
    let mut each_binding: Option<(String, String)> = None;
    let mut condition: Option<(ConditionExpression, bool)> = None;
    let mut switch_expression: Option<TemplateExpression> = None;
    let mut case_expression: Option<TemplateExpression> = None;
    let mut default_case = false;

    for attr in attrs {
        let name = attr.name.local.to_string();
        if name.starts_with("xmlns:") {
            continue;
        }

        let value = attr.value.to_string();
        if let Some(directive) = name.strip_prefix("coil:") {
            match directive {
                "fragment" => {}
                "text" => text_expression = Some(parse_template_expression(&value)?),
                "t" => text_expression = Some(parse_translation_expression(&value)?),
                "utext" => raw_text_expression = Some(parse_template_expression(&value)?),
                "replace" => {
                    include_selector = Some(IncludeTarget::Replace(parse_selector_ref(&value)?))
                }
                "replace-fragment" => include_expression = Some(parse_template_expression(&value)?),
                "include" => {
                    include_selector = Some(IncludeTarget::Insert(parse_selector_ref(&value)?))
                }
                "include-fragment" | "insert-fragment" => {
                    include_expression = Some(parse_template_expression(&value)?)
                }
                "insert" => {
                    let selector = parse_selector_ref(&value)?;
                    if selector.template.is_none() {
                        if let Some(fragment) = selector.fragment {
                            slot_name = Some(fragment);
                        }
                    } else {
                        include_selector = Some(IncludeTarget::Insert(selector));
                    }
                }
                "slot" => slot_name = Some(parse_slot_name(&value)),
                "attr" => dynamic_attrs.extend(parse_attr_bindings(&value)?),
                "with" => with_bindings = parse_with_bindings(&value)?,
                "if" => condition = Some((parse_condition(&value)?, false)),
                "unless" => condition = Some((parse_condition(&value)?, true)),
                "switch" => switch_expression = Some(parse_template_expression(&value)?),
                "case" => case_expression = Some(parse_template_expression(&value)?),
                "default" => default_case = true,
                "each" => each_binding = Some(parse_each_expression(&value)?),
                other => dynamic_attrs.push(AttributeNode::dynamic_expression(
                    other,
                    parse_template_expression(&value)?,
                )?),
            }
            continue;
        }

        static_attrs.push(AttributeNode::static_value(name, value)?);
    }

    let mut rendered_children = render_children(children, path)?;
    if case_expression.is_some() && default_case {
        return Err(TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: "coil:case and coil:default cannot be used on the same element".to_string(),
        });
    }
    if case_expression.is_some() || default_case {
        if switch_expression.is_some() {
            return Err(TemplateModelError::ParseError {
                line: 0,
                column: 0,
                message: "coil:switch cannot be combined with coil:case or coil:default on the same element".to_string(),
            });
        }
    }
    if include_selector.is_some() && include_expression.is_some() {
        return Err(TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: "static fragment inclusion cannot be combined with expression-based fragment inclusion".to_string(),
        });
    }

    if let Some(expression) = text_expression {
        rendered_children = vec![Node::expression(expression)];
    } else if let Some(expression) = raw_text_expression {
        rendered_children = vec![Node::raw_expression(expression)];
    }

    if let Some(expression) = switch_expression {
        rendered_children = vec![build_switch_node(expression, rendered_children)?];
    }

    let mut element = if tag.eq_ignore_ascii_case("coil:block") {
        None
    } else {
        Some(build_element_node(
            tag,
            rendered_children.clone(),
            static_attrs,
            dynamic_attrs,
        )?)
    };

    let mut nodes = match (slot_name, include_selector, include_expression, element.take()) {
        (Some(slot), _, _, Some(mut element)) => {
            element.children = vec![Node::Slot(
                SlotNode::new(SlotName::new(slot)?).with_fallback(rendered_children),
            )];
            vec![Node::Element(element)]
        }
        (Some(slot), _, _, None) => vec![Node::Slot(
            SlotNode::new(SlotName::new(slot)?).with_fallback(rendered_children),
        )],
        (None, Some(IncludeTarget::Replace(selector)), _, _) => {
            vec![Node::include(selector_to_template_selector(selector)?)]
        }
        (None, Some(IncludeTarget::Insert(selector)), _, Some(mut element)) => {
            element.children = vec![Node::include(selector_to_template_selector(selector)?)];
            vec![Node::Element(element)]
        }
        (None, Some(IncludeTarget::Insert(selector)), _, None) => {
            vec![Node::include(selector_to_template_selector(selector)?)]
        }
        (None, None, Some(expression), Some(mut element)) => {
            element.children = vec![Node::include_expression(expression)];
            vec![Node::Element(element)]
        }
        (None, None, Some(expression), None) => vec![Node::include_expression(expression)],
        (None, None, None, Some(element)) => vec![Node::Element(element)],
        (None, None, None, None) => rendered_children,
    };

    if let Some((condition, negated)) = condition {
        nodes = vec![match (condition, negated) {
            (ConditionExpression::Key(key), false) => Node::conditional(key, nodes)?,
            (ConditionExpression::Key(key), true) => Node::conditional_not(key, nodes)?,
            (ConditionExpression::Literal(value), false) => Node::conditional_literal(value, nodes),
            (ConditionExpression::Literal(value), true) => Node::conditional_literal(!value, nodes),
            (ConditionExpression::Expression(expression), false) => {
                Node::conditional_expression(expression, nodes)
            }
            (ConditionExpression::Expression(expression), true) => {
                Node::conditional_expression_not(expression, nodes)
            }
        }];
    }

    if let Some((item, collection)) = each_binding {
        nodes = vec![Node::each(item, collection, nodes)?];
    }

    if !with_bindings.is_empty() {
        nodes = vec![Node::with(with_bindings, nodes)];
    }

    if let Some(expression) = case_expression {
        nodes = vec![Node::case(expression, nodes)];
    } else if default_case {
        nodes = vec![Node::default(nodes)];
    }

    Ok(nodes)
}

fn build_element_node(
    tag: &str,
    children: Vec<Node>,
    static_attrs: Vec<AttributeNode>,
    dynamic_attrs: Vec<AttributeNode>,
) -> Result<ElementNode, TemplateModelError> {
    let mut element = ElementNode::new(tag, children)?;
    element.attributes = Vec::with_capacity(static_attrs.len() + dynamic_attrs.len());
    for attribute in static_attrs {
        push_attribute(&mut element.attributes, attribute);
    }
    for attribute in dynamic_attrs {
        push_attribute(&mut element.attributes, attribute);
    }
    Ok(element)
}

fn push_attribute(attributes: &mut Vec<AttributeNode>, attribute: AttributeNode) {
    if let Some(existing) = attributes
        .iter_mut()
        .find(|existing| existing.name == attribute.name)
    {
        *existing = attribute;
        return;
    }

    attributes.push(attribute);
}

fn render_children<'a>(
    children: impl Iterator<Item = &'a Handle>,
    path: &Path,
) -> Result<Vec<Node>, TemplateModelError> {
    let mut rendered = Vec::new();
    for child in children {
        rendered.extend(render_node(child, path)?);
    }
    Ok(rendered)
}

#[derive(Debug, Clone)]
enum IncludeTarget {
    Replace(TemplateSelectorParts),
    Insert(TemplateSelectorParts),
}

#[derive(Debug, Clone)]
struct TemplateSelectorParts {
    template: Option<String>,
    fragment: Option<String>,
}

fn selector_to_template_selector(
    selector: TemplateSelectorParts,
) -> Result<TemplateSelector, TemplateModelError> {
    let template = selector
        .template
        .ok_or_else(|| TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: match selector.fragment {
                Some(fragment) => {
                    format!("selector is missing a template name before `::{fragment}`")
                }
                None => "selector is missing a template name".to_string(),
            },
        })?;
    Ok(TemplateSelector::new(TemplateName::new(template)?))
}

fn parse_selector_ref(value: &str) -> Result<TemplateSelectorParts, TemplateModelError> {
    let trimmed = value.trim();
    let trimmed = trimmed
        .strip_prefix("~{")
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(trimmed);
    let (template, fragment) = trimmed.split_once("::").unwrap_or((trimmed, ""));
    let template = template.trim();
    let fragment = fragment.trim();

    Ok(TemplateSelectorParts {
        template: (!template.is_empty()).then(|| template.to_string()),
        fragment: (!fragment.is_empty()).then(|| fragment.to_string()),
    })
}

fn parse_render_key(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .or_else(|| {
            trimmed
                .strip_prefix("#{")
                .and_then(|value| value.strip_suffix('}'))
        })
        .or_else(|| {
            trimmed
                .strip_prefix("*{")
                .and_then(|value| value.strip_suffix('}'))
        })
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

fn parse_slot_name(value: &str) -> String {
    parse_render_key(value)
}

fn parse_condition(value: &str) -> Result<ConditionExpression, TemplateModelError> {
    let value = value.trim();
    match parse_template_expression(value)? {
        expression @ TemplateExpression::Compare { .. } => Ok(ConditionExpression::Expression(expression)),
        TemplateExpression::LiteralBool(value) => Ok(ConditionExpression::Literal(value)),
        TemplateExpression::LiteralText(value) => match value.to_ascii_lowercase().as_str() {
            "true" => Ok(ConditionExpression::Literal(true)),
            "false" => Ok(ConditionExpression::Literal(false)),
            _ => Ok(ConditionExpression::Key(value)),
        },
        TemplateExpression::ModelKey(value) | TemplateExpression::AssetPath(value) => {
            Ok(ConditionExpression::Key(value))
        }
        TemplateExpression::TranslationKey(_) => Err(TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: "translation expressions are not valid in coil:if or coil:unless".to_string(),
        }),
    }
}

fn parse_each_expression(value: &str) -> Result<(String, String), TemplateModelError> {
    let (item, collection) =
        value
            .split_once(':')
            .ok_or_else(|| TemplateModelError::ParseError {
                line: 0,
                column: 0,
                message: format!("invalid coil:each expression `{value}`"),
            })?;
    Ok((
        validate_token("render_key", item.trim().to_string())?,
        parse_render_key(collection),
    ))
}

fn parse_with_bindings(value: &str) -> Result<Vec<TemplateBinding>, TemplateModelError> {
    let mut bindings = Vec::new();
    for assignment in value.split(',') {
        let assignment = assignment.trim();
        if assignment.is_empty() {
            continue;
        }

        let (key, raw_value) =
            assignment
                .split_once('=')
                .ok_or_else(|| TemplateModelError::ParseError {
                    line: 0,
                    column: 0,
                    message: format!("invalid coil:with binding `{assignment}`"),
                })?;
        bindings.push(TemplateBinding::new(
            key.trim(),
            parse_template_expression(raw_value.trim())?,
        )?);
    }
    Ok(bindings)
}

fn parse_attr_bindings(value: &str) -> Result<Vec<AttributeNode>, TemplateModelError> {
    let mut attributes = Vec::new();
    for assignment in value.split(',') {
        let assignment = assignment.trim();
        if assignment.is_empty() {
            continue;
        }

        let (name, raw_value) =
            assignment
                .split_once('=')
                .ok_or_else(|| TemplateModelError::ParseError {
                    line: 0,
                    column: 0,
                    message: format!("invalid coil:attr binding `{assignment}`"),
                })?;
        attributes.push(AttributeNode::dynamic_expression(
            name.trim(),
            parse_template_expression(raw_value.trim())?,
        )?);
    }
    Ok(attributes)
}

fn parse_template_expression(value: &str) -> Result<TemplateExpression, TemplateModelError> {
    let trimmed = value.trim();

    if let Some((left, operator, right)) = split_comparison_expression(trimmed) {
        return Ok(TemplateExpression::Compare {
            left: Box::new(parse_template_expression(left.trim())?),
            operator,
            right: Box::new(parse_template_expression(right.trim())?),
        });
    }

    if let Some(inner) = trimmed
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .or_else(|| {
            trimmed
                .strip_prefix("#{")
                .and_then(|value| value.strip_suffix('}'))
        })
        .or_else(|| {
            trimmed
                .strip_prefix("*{")
                .and_then(|value| value.strip_suffix('}'))
        })
    {
        return parse_template_expression(inner.trim());
    }

    if let Some(inner) = trimmed
        .strip_prefix("@{")
        .and_then(|value| value.strip_suffix('}'))
    {
        let inner = inner.trim();
        return Ok(TemplateExpression::AssetPath(inner.to_string()));
    }

    if let Some(inner) = trimmed
        .strip_prefix("asset(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let inner = inner.trim().trim_matches('"').trim_matches('\'');
        return Ok(TemplateExpression::AssetPath(inner.to_string()));
    }

    if let Some(inner) = trimmed
        .strip_prefix("t(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let inner = inner.trim();
        let key = inner
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .or_else(|| inner.strip_prefix('\'').and_then(|value| value.strip_suffix('\'')))
            .ok_or_else(|| TemplateModelError::ParseError {
                line: 0,
                column: 0,
                message: format!(
                    "translation helper expects a quoted key like t('checkout.title'), got `{trimmed}`"
                ),
            })?;
        return Ok(TemplateExpression::TranslationKey(validate_token(
            "translation_key",
            key.to_string(),
        )?));
    }

    if let Some(inner) = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Ok(TemplateExpression::LiteralText(inner.to_string()));
    }
    if let Some(inner) = trimmed
        .strip_prefix('\'')
        .and_then(|value| value.strip_suffix('\''))
    {
        return Ok(TemplateExpression::LiteralText(inner.to_string()));
    }

    match trimmed {
        "true" => Ok(TemplateExpression::LiteralBool(true)),
        "false" => Ok(TemplateExpression::LiteralBool(false)),
        other => Ok(TemplateExpression::ModelKey(other.to_string())),
    }
}

fn split_comparison_expression(value: &str) -> Option<(&str, ComparisonOperator, &str)> {
    let mut in_single = false;
    let mut in_double = false;
    let mut paren_depth = 0usize;
    let mut brace_depth = 0usize;
    let chars: Vec<(usize, char)> = value.char_indices().collect();
    let mut index = 0usize;
    while index < chars.len() {
        let (byte_index, ch) = chars[index];
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '(' if !in_single && !in_double => paren_depth += 1,
            ')' if !in_single && !in_double && paren_depth > 0 => paren_depth -= 1,
            '{' if !in_single && !in_double => brace_depth += 1,
            '}' if !in_single && !in_double && brace_depth > 0 => brace_depth -= 1,
            '=' if !in_single && !in_double && paren_depth == 0 && brace_depth == 0 => {
                if value[byte_index..].starts_with("==") {
                    let left = &value[..byte_index];
                    let right = &value[byte_index + 2..];
                    return Some((left, ComparisonOperator::Equal, right));
                }
            }
            '!' if !in_single && !in_double && paren_depth == 0 && brace_depth == 0 => {
                if value[byte_index..].starts_with("!=") {
                    let left = &value[..byte_index];
                    let right = &value[byte_index + 2..];
                    return Some((left, ComparisonOperator::NotEqual, right));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn build_switch_node(
    expression: TemplateExpression,
    children: Vec<Node>,
) -> Result<Node, TemplateModelError> {
    let mut cases = Vec::new();
    let mut default = None;

    for child in children {
        match child {
            Node::Case {
                expression,
                children,
            } => cases.push(SwitchCaseNode {
                expression,
                children,
            }),
            Node::Default { children } => {
                if default.is_some() {
                    return Err(TemplateModelError::ParseError {
                        line: 0,
                        column: 0,
                        message: "coil:switch may only contain one coil:default branch".to_string(),
                    });
                }
                default = Some(children);
            }
            other => {
                return Err(TemplateModelError::ParseError {
                    line: 0,
                    column: 0,
                    message: format!(
                        "coil:switch only accepts direct children annotated with coil:case or coil:default, found `{:?}`",
                        other
                    ),
                });
            }
        }
    }

    Ok(Node::switch(expression, cases, default))
}

fn parse_translation_expression(value: &str) -> Result<TemplateExpression, TemplateModelError> {
    let trimmed = value.trim();
    let is_wrapped_expression =
        trimmed.starts_with("${") || trimmed.starts_with("#{") || trimmed.starts_with("*{");
    match parse_template_expression(trimmed)? {
        TemplateExpression::TranslationKey(key) => Ok(TemplateExpression::TranslationKey(key)),
        TemplateExpression::ModelKey(key) if !is_wrapped_expression => Ok(
            TemplateExpression::TranslationKey(validate_token("translation_key", key)?),
        ),
        _ => Err(TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: format!(
                "coil:t expects a translation key like `home.title` or `t('home.title')`, got `{trimmed}`"
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_root(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("coil-template-parser-{label}-{unique}"))
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn loads_templates_from_app_template_tree() {
        let root = unique_root("load");
        write_file(
            &root.join("templates/layouts/base.html"),
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs" coil:fragment="shell">
  <body>
    <main coil:insert="~{::content}">
      <section coil:fragment="content"><p>Fallback</p></section>
    </main>
  </body>
</html>"#,
        );
        write_file(
            &root.join("templates/components/hero.html"),
            r#"<section class="hero" xmlns:coil="https://coil.rs" coil:fragment="hero">Hero</section>"#,
        );

        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let parser = TemplateSourceParser::new();
        let templates = parser
            .load_directory(root.join("templates"), namespace)
            .unwrap();

        assert_eq!(templates.len(), 2);
        assert!(
            templates
                .iter()
                .any(|template| template.key.name.as_str() == "layouts/base")
        );
        assert!(
            templates
                .iter()
                .any(|template| template.key.name.as_str() == "components/hero")
        );
    }

    #[test]
    fn parses_thymeleaf_directives_into_template_nodes() {
        let root = unique_root("parse");
        let path = root.join("templates/pages/home.html");
        write_file(
            &path,
            r#"<!doctype html>
<html xmlns:coil="https://coil.rs" coil:with="page_title='Shoppr'">
  <head>
    <title coil:text="${page_title}">Fallback</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="${asset('theme/assets/site.css')}" />
  </head>
  <body>
    <section class="home-page" coil:fragment="content">
      <div coil:replace="~{components/hero :: hero}"></div>
      <div coil:replace="~{commerce/collection-grid :: grid}"></div>
    </section>
  </body>
</html>"#,
        );

        let parser = TemplateSourceParser::new();
        let template = parser
            .parse_file(
                root.join("templates"),
                &path,
                TemplateNamespace::new("customer-app").unwrap(),
            )
            .unwrap();

        assert_eq!(template.kind, TemplateKind::Layout);
        assert_eq!(template.nodes.len(), 2);
        assert!(matches!(template.nodes.first(), Some(Node::StaticText(_))));
        match template.nodes.get(1) {
            Some(Node::With { children, .. }) => {
                assert!(matches!(children.first(), Some(Node::Element(_))));
            }
            other => panic!("expected a `coil:with` wrapper, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_each_expressions() {
        let error = parse_each_expression("collection").unwrap_err();
        assert!(matches!(error, TemplateModelError::ParseError { .. }));
    }

    #[test]
    fn parses_translation_expressions_in_text_and_attributes() {
        let root = unique_root("translations");
        let path = root.join("templates/pages/home.html");
        write_file(
            &path,
            r#"<section xmlns:coil="https://coil.rs" coil:fragment="home">
  <h1 coil:text="t('home.title')">Fallback</h1>
  <a coil:title="${t('home.cta')}">Link</a>
</section>"#,
        );

        let parser = TemplateSourceParser::new();
        let template = parser
            .parse_file(
                root.join("templates"),
                &path,
                TemplateNamespace::new("customer-app").unwrap(),
            )
            .unwrap();

        fn contains_translation_node(nodes: &[Node], key: &str) -> bool {
            nodes.iter().any(|node| match node {
                Node::Expression(TemplateExpression::TranslationKey(found)) => found == key,
                Node::RawExpression(TemplateExpression::TranslationKey(found)) => found == key,
                Node::Expression(_) | Node::RawExpression(_) => false,
                Node::Element(element) => {
                    element.attributes.iter().any(|attribute| {
                        attribute.value
                            == AttributeValue::DynamicExpression(
                                TemplateExpression::TranslationKey(key.to_string()),
                            )
                    }) || contains_translation_node(&element.children, key)
                }
                Node::With { children, .. }
                | Node::Conditional { children, .. }
                | Node::Each { children, .. }
                | Node::Default { children } => contains_translation_node(children, key),
                Node::Switch { cases, default, .. } => {
                    cases.iter().any(|case| contains_translation_node(&case.children, key))
                        || default
                            .as_ref()
                            .is_some_and(|children| contains_translation_node(children, key))
                }
                Node::Case { children, .. } => contains_translation_node(children, key),
                Node::Slot(slot) => slot
                    .fallback
                    .as_ref()
                    .is_some_and(|children| contains_translation_node(children, key)),
                Node::StaticText(_)
                | Node::Value(_)
                | Node::RawValue(_)
                | Node::Include(_)
                | Node::IncludeExpression(_) => false,
            })
        }

        assert!(contains_translation_node(&template.nodes, "home.title"));
        assert!(contains_translation_node(&template.nodes, "home.cta"));
    }

    #[test]
    fn parses_dv_t_as_a_first_class_translation_directive() {
        let root = unique_root("directive-translations");
        let path = root.join("templates/pages/home.html");
        write_file(
            &path,
            r#"<section xmlns:coil="https://coil.rs" coil:fragment="home">
  <h1 coil:t="home.title">Fallback</h1>
  <p coil:t="${t('home.summary')}">Fallback</p>
</section>"#,
        );

        let parser = TemplateSourceParser::new();
        let template = parser
            .parse_file(
                root.join("templates"),
                &path,
                TemplateNamespace::new("customer-app").unwrap(),
            )
            .unwrap();

        fn contains_translation_node(nodes: &[Node], key: &str) -> bool {
            nodes.iter().any(|node| match node {
                Node::Expression(TemplateExpression::TranslationKey(found)) => found == key,
                Node::RawExpression(TemplateExpression::TranslationKey(found)) => found == key,
                Node::Expression(_) | Node::RawExpression(_) => false,
                Node::Element(element) => contains_translation_node(&element.children, key),
                Node::With { children, .. }
                | Node::Conditional { children, .. }
                | Node::Each { children, .. }
                | Node::Default { children } => contains_translation_node(children, key),
                Node::Switch { cases, default, .. } => {
                    cases.iter().any(|case| contains_translation_node(&case.children, key))
                        || default
                            .as_ref()
                            .is_some_and(|children| contains_translation_node(children, key))
                }
                Node::Case { children, .. } => contains_translation_node(children, key),
                Node::Slot(slot) => slot
                    .fallback
                    .as_ref()
                    .is_some_and(|children| contains_translation_node(children, key)),
                Node::StaticText(_)
                | Node::Value(_)
                | Node::RawValue(_)
                | Node::Include(_)
                | Node::IncludeExpression(_) => false,
            })
        }

        assert!(contains_translation_node(&template.nodes, "home.title"));
        assert!(contains_translation_node(&template.nodes, "home.summary"));
    }

    #[test]
    fn rejects_non_translation_expressions_in_dv_t() {
        let error = parse_translation_expression("${headline}").unwrap_err();
        assert_eq!(
            error,
            TemplateModelError::ParseError {
                line: 0,
                column: 0,
                message:
                    "coil:t expects a translation key like `home.title` or `t('home.title')`, got `${headline}`"
                        .to_string(),
            }
        );
    }
}
