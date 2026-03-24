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
        let kind = if source.contains("dv:fragment=") {
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
        let source = fs::read_to_string(path).map_err(|error| TemplateModelError::TemplateRead {
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
    let mut slot_name: Option<String> = None;
    let mut text_key: Option<String> = None;
    let mut raw_text_key: Option<String> = None;
    let mut with_bindings: Vec<TemplateBinding> = Vec::new();
    let mut each_binding: Option<(String, String)> = None;
    let mut condition: Option<(ConditionExpression, bool)> = None;

    for attr in attrs {
        let name = attr.name.local.to_string();
        if name.starts_with("xmlns:") {
            continue;
        }

        let value = attr.value.to_string();
        if let Some(directive) = name.strip_prefix("dv:") {
            match directive {
                "fragment" => {}
                "text" => text_key = Some(parse_render_key(&value)),
                "utext" => raw_text_key = Some(parse_render_key(&value)),
                "replace" => include_selector = Some(IncludeTarget::Replace(parse_selector_ref(&value)?)),
                "include" => include_selector = Some(IncludeTarget::Insert(parse_selector_ref(&value)?)),
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
                "each" => each_binding = Some(parse_each_expression(&value)?),
                other => {
                    dynamic_attrs.push(AttributeNode::dynamic_expression(
                        other,
                        parse_template_expression(&value)?,
                    )?)
                }
            }
            continue;
        }

        static_attrs.push(AttributeNode::static_value(name, value)?);
    }

    let mut rendered_children = render_children(children, path)?;
    if let Some(key) = text_key {
        rendered_children = vec![Node::value(key)?];
    } else if let Some(key) = raw_text_key {
        rendered_children = vec![Node::raw_value(key)?];
    }

    let mut element = if tag.eq_ignore_ascii_case("dv:block") {
        None
    } else {
        Some(build_element_node(
            tag,
            rendered_children.clone(),
            static_attrs,
            dynamic_attrs,
        )?)
    };

    let mut nodes = match (slot_name, include_selector, element.take()) {
        (Some(slot), _, Some(mut element)) => {
            element.children = vec![Node::Slot(
                SlotNode::new(SlotName::new(slot)?).with_fallback(rendered_children),
            )];
            vec![Node::Element(element)]
        }
        (Some(slot), _, None) => vec![Node::Slot(
            SlotNode::new(SlotName::new(slot)?).with_fallback(rendered_children),
        )],
        (None, Some(IncludeTarget::Replace(selector)), _) => {
            vec![Node::include(selector_to_template_selector(selector)?)]
        }
        (None, Some(IncludeTarget::Insert(selector)), Some(mut element)) => {
            element.children = vec![Node::include(selector_to_template_selector(selector)?)];
            vec![Node::Element(element)]
        }
        (None, Some(IncludeTarget::Insert(selector)), None) => {
            vec![Node::include(selector_to_template_selector(selector)?)]
        }
        (None, None, Some(element)) => vec![Node::Element(element)],
        (None, None, None) => rendered_children,
    };

    if let Some((condition, negated)) = condition {
        nodes = vec![match (condition, negated) {
            (ConditionExpression::Key(key), false) => Node::conditional(key, nodes)?,
            (ConditionExpression::Key(key), true) => Node::conditional_not(key, nodes)?,
            (ConditionExpression::Literal(value), false) => Node::conditional_literal(value, nodes),
            (ConditionExpression::Literal(value), true) => {
                Node::conditional_literal(!value, nodes)
            }
        }];
    }

    if let Some((item, collection)) = each_binding {
        nodes = vec![Node::each(item, collection, nodes)?];
    }

    if !with_bindings.is_empty() {
        nodes = vec![Node::with(with_bindings, nodes)];
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
    if let Some(existing) = attributes.iter_mut().find(|existing| existing.name == attribute.name)
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
        .or_else(|| trimmed.strip_prefix("#{").and_then(|value| value.strip_suffix('}')))
        .or_else(|| trimmed.strip_prefix("*{").and_then(|value| value.strip_suffix('}')))
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
        TemplateExpression::LiteralBool(value) => Ok(ConditionExpression::Literal(value)),
        TemplateExpression::LiteralText(value) => match value.to_ascii_lowercase().as_str() {
            "true" => Ok(ConditionExpression::Literal(true)),
            "false" => Ok(ConditionExpression::Literal(false)),
            _ => Ok(ConditionExpression::Key(value)),
        },
        TemplateExpression::ModelKey(value) | TemplateExpression::AssetPath(value) => {
            Ok(ConditionExpression::Key(value))
        }
    }
}

fn parse_each_expression(value: &str) -> Result<(String, String), TemplateModelError> {
    let (item, collection) = value.split_once(':').ok_or_else(|| TemplateModelError::ParseError {
        line: 0,
        column: 0,
        message: format!("invalid dv:each expression `{value}`"),
    })?;
    Ok((validate_token("render_key", item.trim().to_string())?, parse_render_key(collection)))
}

fn parse_with_bindings(value: &str) -> Result<Vec<TemplateBinding>, TemplateModelError> {
    let mut bindings = Vec::new();
    for assignment in value.split(',') {
        let assignment = assignment.trim();
        if assignment.is_empty() {
            continue;
        }

        let (key, raw_value) = assignment.split_once('=').ok_or_else(|| TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: format!("invalid dv:with binding `{assignment}`"),
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

        let (name, raw_value) = assignment.split_once('=').ok_or_else(|| TemplateModelError::ParseError {
            line: 0,
            column: 0,
            message: format!("invalid dv:attr binding `{assignment}`"),
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

    if let Some(inner) = trimmed
        .strip_prefix("${")
        .and_then(|value| value.strip_suffix('}'))
        .or_else(|| trimmed.strip_prefix("#{").and_then(|value| value.strip_suffix('}')))
        .or_else(|| trimmed.strip_prefix("*{").and_then(|value| value.strip_suffix('}')))
    {
        return Ok(TemplateExpression::ModelKey(inner.trim().to_string()));
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
        .strip_prefix("@{")
        .and_then(|value| value.strip_suffix('}'))
    {
        return Ok(TemplateExpression::AssetPath(inner.trim().to_string()));
    }

    if let Some(inner) = trimmed.strip_prefix('"').and_then(|value| value.strip_suffix('"')) {
        return Ok(TemplateExpression::LiteralText(inner.to_string()));
    }
    if let Some(inner) = trimmed.strip_prefix('\'').and_then(|value| value.strip_suffix('\'')) {
        return Ok(TemplateExpression::LiteralText(inner.to_string()));
    }

    match trimmed {
        "true" => Ok(TemplateExpression::LiteralBool(true)),
        "false" => Ok(TemplateExpression::LiteralBool(false)),
        other => Ok(TemplateExpression::ModelKey(other.to_string())),
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
        std::env::temp_dir().join(format!("davenda-template-parser-{label}-{unique}"))
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
<html xmlns:dv="https://davenda.dev" dv:fragment="shell">
  <body>
    <main dv:insert="~{::content}">
      <section dv:fragment="content"><p>Fallback</p></section>
    </main>
  </body>
</html>"#,
        );
        write_file(
            &root.join("templates/components/hero.html"),
            r#"<section class="hero" xmlns:dv="https://davenda.dev" dv:fragment="hero">Hero</section>"#,
        );

        let namespace = TemplateNamespace::new("customer-app").unwrap();
        let parser = TemplateSourceParser::new();
        let templates = parser.load_directory(root.join("templates"), namespace).unwrap();

        assert_eq!(templates.len(), 2);
        assert!(templates
            .iter()
            .any(|template| template.key.name.as_str() == "layouts/base"));
        assert!(templates
            .iter()
            .any(|template| template.key.name.as_str() == "components/hero"));
    }

    #[test]
    fn parses_thymeleaf_directives_into_template_nodes() {
        let root = unique_root("parse");
        let path = root.join("templates/pages/home.html");
        write_file(
            &path,
            r#"<!doctype html>
<html xmlns:dv="https://davenda.dev" dv:with="pageTitle='Harbor Shop'">
  <head>
    <title dv:text="${pageTitle}">Fallback</title>
    <link rel="stylesheet" href="/theme/assets/site.css" dv:href="${asset('theme/assets/site.css')}" />
  </head>
  <body>
    <section class="home-page" dv:fragment="content">
      <div dv:replace="~{components/hero :: hero}"></div>
      <div dv:replace="~{commerce/collection-grid :: grid}"></div>
    </section>
  </body>
</html>"#,
        );

        let parser = TemplateSourceParser::new();
        let template = parser
            .parse_file(root.join("templates"), &path, TemplateNamespace::new("customer-app").unwrap())
            .unwrap();

        assert_eq!(template.kind, TemplateKind::Layout);
        assert_eq!(template.nodes.len(), 2);
        assert!(matches!(template.nodes.first(), Some(Node::StaticText(_))));
        match template.nodes.get(1) {
            Some(Node::With { children, .. }) => {
                assert!(matches!(children.first(), Some(Node::Element(_))));
            }
            other => panic!("expected a `dv:with` wrapper, got {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_each_expressions() {
        let error = parse_each_expression("collection").unwrap_err();
        assert!(matches!(error, TemplateModelError::ParseError { .. }));
    }
}
