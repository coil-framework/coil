use super::*;
use davenda_i18n::{I18nError, LocaleTag, LocalizedUrls};
use davenda_seo::{
    HeadMetadata, OpenGraphData, OpenGraphType, RobotsDirective, SeoError, page_node,
};
use davenda_template::{
    AttributeNode, DocumentRenderRequest, ElementNode, FragmentRenderRequest, Node, RenderModel,
    RenderValue, TemplateDefinition, TemplateKind, TemplateModelError, TemplateName,
    TemplateNamespace, TemplateRuntime, TemplateSelector, TrustedHtml,
};
use davenda_wasm::{RobotsDirective as TypedRobotsDirective, TypedMetadata};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeRenderError {
    #[error(transparent)]
    Template(#[from] TemplateModelError),
    #[error(transparent)]
    I18n(#[from] I18nError),
    #[error(transparent)]
    Seo(#[from] SeoError),
    #[error(transparent)]
    RouteUrl(#[from] RouteUrlError),
}

impl RuntimePlan {
    pub fn render_page_response(
        &self,
        execution: &RequestExecution,
        page: &PageResponse,
        extra_metadata: Option<&TypedMetadata>,
    ) -> Result<String, RuntimeRenderError> {
        let selector = template_selector(&page.template)?;
        let namespaces = self.template_namespaces_for_execution(execution);
        let model = self.render_model_for_execution(execution, &page.template, None)?;

        let html = match self.template.runtime.render_document(
            &namespaces,
            DocumentRenderRequest::new(selector.clone(), model.clone()),
        ) {
            Ok(output) => output.html,
            Err(TemplateModelError::TemplateNotFound { .. })
            | Err(TemplateModelError::TemplateKindMismatch {
                actual: TemplateKind::Fragment,
                ..
            }) => {
                let content =
                    self.render_fragment_content(execution, &namespaces, &selector, model, None)?;
                self.render_document_shell(execution, &page.template, content)?
            }
            Err(error) => return Err(error.into()),
        };

        self.decorate_page_document(execution, &page.template, html, extra_metadata)
    }

    pub fn render_fragment_response(
        &self,
        execution: &RequestExecution,
        fragment: &FragmentResponse,
    ) -> Result<String, RuntimeRenderError> {
        let selector = template_selector(&fragment.template)?;
        let namespaces = self.template_namespaces_for_execution(execution);
        let model = self.render_model_for_execution(
            execution,
            &fragment.template,
            Some(fragment.fragment_id.as_str()),
        )?;

        self.render_fragment_content(
            execution,
            &namespaces,
            &selector,
            model,
            Some(fragment.fragment_id.as_str()),
        )
        .map_err(Into::into)
    }

    fn render_fragment_content(
        &self,
        execution: &RequestExecution,
        namespaces: &[TemplateNamespace],
        selector: &TemplateSelector,
        model: RenderModel,
        fragment_id: Option<&str>,
    ) -> Result<String, TemplateModelError> {
        match self.template.runtime.render_fragment(
            namespaces,
            FragmentRenderRequest::new(selector.clone(), model.clone()),
        ) {
            Ok(output) => Ok(output.html),
            Err(TemplateModelError::TemplateNotFound { .. }) => {
                let runtime = self.synthetic_template_runtime(execution, selector.name(), false)?;
                Ok(runtime
                    .render_fragment(
                        namespaces,
                        FragmentRenderRequest::new(selector.clone(), model),
                    )?
                    .html)
            }
            Err(error) => {
                if matches!(
                    error,
                    TemplateModelError::TemplateKindMismatch {
                        actual: TemplateKind::Layout,
                        ..
                    } | TemplateModelError::FragmentCannotRenderLayout { .. }
                ) && fragment_id.is_none()
                {
                    return Ok(self.render_document_shell(
                        execution,
                        selector.name().as_str(),
                        self.template
                            .runtime
                            .render_document(
                                namespaces,
                                DocumentRenderRequest::new(selector.clone(), model),
                            )?
                            .html,
                    )?);
                }

                Err(error)
            }
        }
    }

    fn render_document_shell(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        content: String,
    ) -> Result<String, TemplateModelError> {
        let shell_name = TemplateName::new("runtime.page.shell")?;
        let shell_selector = TemplateSelector::new(shell_name.clone());
        let mut registry = self.template.registry.clone();
        match registry.register(runtime_page_shell_template(
            self.template.customer_app_namespace.clone(),
        )?) {
            Ok(()) | Err(TemplateModelError::DuplicateTemplate { .. }) => {}
            Err(error) => return Err(error),
        }

        let mut model = self.render_model_for_execution(execution, template_name, None)?;
        model = model
            .with_value(
                "page_title",
                RenderValue::text(format!(
                    "{} · {}",
                    execution.route.route_name, execution.customer_app
                )),
            )?
            .with_value(
                "page_content",
                RenderValue::trusted_html(TrustedHtml::new(content)?),
            )?;

        Ok(TemplateRuntime::new(registry)
            .render_document(
                &[self.template.customer_app_namespace.clone()],
                DocumentRenderRequest::new(shell_selector, model),
            )?
            .html)
    }

    fn synthetic_template_runtime(
        &self,
        execution: &RequestExecution,
        template_name: &TemplateName,
        page_layout: bool,
    ) -> Result<TemplateRuntime, TemplateModelError> {
        let mut registry = self.template.registry.clone();
        let namespace = self
            .module_template_namespace(execution)
            .unwrap_or_else(|| self.template.customer_app_namespace.clone());

        let definition = if page_layout {
            runtime_fallback_page_template(namespace, template_name.clone())?
        } else {
            runtime_fallback_fragment_template(namespace, template_name.clone())?
        };

        registry.register(definition)?;
        Ok(TemplateRuntime::new(registry))
    }

    fn template_namespaces_for_execution(
        &self,
        execution: &RequestExecution,
    ) -> Vec<TemplateNamespace> {
        let module_namespace = self.module_template_namespace(execution);
        self.template.namespace_chain(module_namespace.as_ref())
    }

    fn module_template_namespace(&self, execution: &RequestExecution) -> Option<TemplateNamespace> {
        self.http
            .routes
            .iter()
            .find(|route| route.name == execution.route.route_name)
            .and_then(|route| route.module.as_deref())
            .and_then(|module| TemplateNamespace::new(module.to_string()).ok())
    }

    fn render_model_for_execution(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        fragment_id: Option<&str>,
    ) -> Result<RenderModel, TemplateModelError> {
        let mut model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(execution.customer_app.clone()),
            )?
            .with_value(
                "route_name",
                RenderValue::text(execution.route.route_name.clone()),
            )?
            .with_value("path", RenderValue::text(execution.path.clone()))?
            .with_value("locale", RenderValue::text(execution.locale.clone()))?
            .with_value(
                "method",
                RenderValue::text(format!("{:?}", execution.method)),
            )?
            .with_value(
                "template_name",
                RenderValue::text(template_name.to_string()),
            )?
            .with_value(
                "route_area",
                RenderValue::text(format!("{:?}", execution.route_area)),
            )?
            .with_value(
                "request_id",
                RenderValue::text(execution.trace.request_id.clone()),
            )?
            .with_value(
                "transport_scheme",
                RenderValue::text(execution.trace.transport_scheme.clone()),
            )?
            .with_value(
                "principal_id",
                RenderValue::text(
                    execution
                        .principal
                        .principal_id
                        .clone()
                        .unwrap_or_else(|| "anonymous".to_string()),
                ),
            )?
            .with_value(
                "session_id",
                RenderValue::text(
                    execution
                        .session
                        .session_id
                        .clone()
                        .unwrap_or_else(|| "guest".to_string()),
                ),
            )?
            .with_value(
                "surface_id",
                RenderValue::text(
                    fragment_id
                        .map(str::to_string)
                        .unwrap_or_else(|| execution.route.route_name.clone()),
                ),
            )?;

        if let Some(fragment_id) = fragment_id {
            model = model.with_value("fragment_id", RenderValue::text(fragment_id.to_string()))?;
        }

        Ok(model)
    }

    fn decorate_page_document(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        document_html: String,
        extra_metadata: Option<&TypedMetadata>,
    ) -> Result<String, RuntimeRenderError> {
        let metadata =
            self.head_metadata_for_execution(execution, template_name, extra_metadata)?;
        let json_ld = if self.seo.allows_json_ld() {
            vec![page_node(
                metadata.title.clone(),
                metadata.canonical_url.clone(),
                metadata.description.clone(),
            )?]
        } else {
            Vec::new()
        };
        let extra_json_ld = extra_metadata
            .map(|extra_metadata| {
                extra_metadata
                    .json_ld
                    .iter()
                    .map(davenda_wasm::JsonLdNode::render)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let head_markup = render_head_markup(&metadata, &json_ld, &extra_json_ld);
        Ok(inject_head_markup(document_html, &head_markup))
    }

    fn head_metadata_for_execution(
        &self,
        execution: &RequestExecution,
        template_name: &str,
        extra_metadata: Option<&TypedMetadata>,
    ) -> Result<HeadMetadata, RuntimeRenderError> {
        let title = format!(
            "{} · {}",
            execution.route.route_name, execution.customer_app
        );
        let description = format!(
            "{} response for {} using {}",
            execution.route.route_name, execution.customer_app, template_name
        );
        let urls = self.localized_urls_for_execution(execution)?;
        let open_graph = Some(OpenGraphData::new(
            title.clone(),
            description.clone(),
            None,
            OpenGraphType::Website,
        )?);

        let mut metadata = HeadMetadata::new(
            title,
            description,
            urls,
            [RobotsDirective::Index, RobotsDirective::Follow],
            open_graph,
        )?;

        if let Some(extra_metadata) = extra_metadata {
            if let Some(title) = &extra_metadata.title {
                metadata.title = title.clone();
            }
            if let Some(description) = &extra_metadata.description {
                metadata.description = description.clone();
            }
            if let Some(canonical_url) = &extra_metadata.canonical_url {
                metadata.canonical_url = canonical_url.clone();
            }
            metadata
                .alternate_urls
                .extend(extra_metadata.alternate_urls.clone());
            metadata.robots.extend(
                extra_metadata
                    .robots
                    .iter()
                    .map(|directive| match directive {
                        TypedRobotsDirective::Index => RobotsDirective::Index,
                        TypedRobotsDirective::NoIndex => RobotsDirective::NoIndex,
                        TypedRobotsDirective::Follow => RobotsDirective::Follow,
                        TypedRobotsDirective::NoFollow => RobotsDirective::NoFollow,
                        TypedRobotsDirective::NoArchive => RobotsDirective::NoArchive,
                    }),
            );
            metadata.open_graph = Some(OpenGraphData::new(
                metadata.title.clone(),
                metadata.description.clone(),
                None,
                OpenGraphType::Website,
            )?);
        }

        Ok(metadata)
    }

    fn localized_urls_for_execution(
        &self,
        execution: &RequestExecution,
    ) -> Result<LocalizedUrls, RuntimeRenderError> {
        let route = self
            .http
            .routes
            .iter()
            .find(|route| route.name == execution.route.route_name)
            .expect("request execution routes must resolve from the runtime plan");
        let current_locale = LocaleTag::new(execution.locale.clone())?;
        let locales = if route.locale_policy == LocalePolicy::Localized {
            let mut locales = vec![current_locale.clone()];
            locales.extend(
                self.i18n
                    .supported_locales
                    .iter()
                    .filter(|locale| **locale != current_locale)
                    .cloned(),
            );
            locales
        } else {
            vec![self.i18n.default_locale.clone()]
        };
        let canonical = self.http.absolute_url_for(
            &self.config,
            &execution.route.route_name,
            &execution.route.params,
            Some(current_locale.as_str()),
        )?;
        let mut alternate_hreflang = BTreeMap::new();
        for locale in locales {
            alternate_hreflang.insert(
                locale.to_string(),
                self.http.absolute_url_for(
                    &self.config,
                    &execution.route.route_name,
                    &execution.route.params,
                    Some(locale.as_str()),
                )?,
            );
        }

        Ok(LocalizedUrls {
            canonical,
            alternate_hreflang,
        })
    }
}

fn template_selector(template: &str) -> Result<TemplateSelector, TemplateModelError> {
    Ok(TemplateSelector::new(TemplateName::new(
        template.to_string(),
    )?))
}

fn runtime_page_shell_template(
    namespace: TemplateNamespace,
) -> Result<TemplateDefinition, TemplateModelError> {
    let title = ElementNode::new("title", vec![Node::value("page_title")?])?;
    let head = ElementNode::new("head", vec![Node::Element(title)])?;
    let body = ElementNode::new("body", vec![Node::raw_value("page_content")?])?
        .with_attribute(AttributeNode::dynamic_text(
            "data-customer-app",
            "customer_app",
        )?)
        .with_attribute(AttributeNode::dynamic_text("data-route", "route_name")?)
        .with_attribute(AttributeNode::dynamic_text(
            "data-template",
            "template_name",
        )?);
    let html = ElementNode::new("html", vec![Node::Element(head), Node::Element(body)])?
        .with_attribute(AttributeNode::dynamic_text("lang", "locale")?);

    Ok(TemplateDefinition::layout(
        namespace,
        TemplateName::new("runtime.page.shell")?,
        vec![Node::static_text("<!DOCTYPE html>"), Node::Element(html)],
    ))
}

fn runtime_fallback_page_template(
    namespace: TemplateNamespace,
    name: TemplateName,
) -> Result<TemplateDefinition, TemplateModelError> {
    let heading = ElementNode::new("h1", vec![Node::value("route_name")?])?;
    let path = ElementNode::new("p", vec![Node::value("path")?])?.with_attribute(
        AttributeNode::static_value("class", "davenda-runtime-path")?,
    );
    let template = ElementNode::new("p", vec![Node::value("template_name")?])?.with_attribute(
        AttributeNode::static_value("class", "davenda-runtime-template")?,
    );
    let main = ElementNode::new(
        "main",
        vec![
            Node::Element(heading),
            Node::Element(path),
            Node::Element(template),
        ],
    )?
    .with_attribute(AttributeNode::dynamic_text("data-route", "route_name")?)
    .with_attribute(AttributeNode::dynamic_text(
        "data-template",
        "template_name",
    )?);

    let body = ElementNode::new("body", vec![Node::Element(main)])?;
    let html = ElementNode::new("html", vec![Node::Element(body)])?
        .with_attribute(AttributeNode::dynamic_text("lang", "locale")?);

    Ok(TemplateDefinition::layout(
        namespace,
        name,
        vec![Node::static_text("<!DOCTYPE html>"), Node::Element(html)],
    ))
}

fn runtime_fallback_fragment_template(
    namespace: TemplateNamespace,
    name: TemplateName,
) -> Result<TemplateDefinition, TemplateModelError> {
    let heading = ElementNode::new("strong", vec![Node::value("route_name")?])?;
    let path = ElementNode::new("span", vec![Node::value("path")?])?;
    let container = ElementNode::new(
        "div",
        vec![
            Node::Element(heading),
            Node::static_text(" "),
            Node::Element(path),
        ],
    )?
    .with_attribute(AttributeNode::dynamic_text("id", "surface_id")?)
    .with_attribute(AttributeNode::dynamic_text(
        "data-template",
        "template_name",
    )?)
    .with_attribute(AttributeNode::dynamic_text("data-locale", "locale")?);

    Ok(TemplateDefinition::fragment(
        namespace,
        name,
        vec![Node::Element(container)],
    ))
}

fn render_head_markup(
    metadata: &HeadMetadata,
    json_ld: &[davenda_seo::JsonLdNode],
    extra_json_ld: &[String],
) -> String {
    let mut markup = String::new();
    markup.push_str(&format!(
        "<meta name=\"description\" content=\"{}\">",
        escape_html_attribute(&metadata.description)
    ));
    markup.push_str(&format!(
        "<link rel=\"canonical\" href=\"{}\">",
        escape_html_attribute(&metadata.canonical_url)
    ));
    if !metadata.robots.is_empty() {
        markup.push_str(&format!(
            "<meta name=\"robots\" content=\"{}\">",
            escape_html_attribute(&metadata.robots_content())
        ));
    }
    for (locale, url) in &metadata.alternate_urls {
        markup.push_str(&format!(
            "<link rel=\"alternate\" hreflang=\"{}\" href=\"{}\">",
            escape_html_attribute(locale),
            escape_html_attribute(url)
        ));
    }
    if let Some(open_graph) = &metadata.open_graph {
        markup.push_str(&format!(
            "<meta property=\"og:title\" content=\"{}\">",
            escape_html_attribute(&open_graph.title)
        ));
        markup.push_str(&format!(
            "<meta property=\"og:description\" content=\"{}\">",
            escape_html_attribute(&open_graph.description)
        ));
        markup.push_str(&format!(
            "<meta property=\"og:type\" content=\"{}\">",
            open_graph.graph_type
        ));
        if let Some(image_url) = &open_graph.image_url {
            markup.push_str(&format!(
                "<meta property=\"og:image\" content=\"{}\">",
                escape_html_attribute(image_url)
            ));
        }
    }
    for node in json_ld {
        markup.push_str(&format!(
            "<script type=\"application/ld+json\">{}</script>",
            node.render()
        ));
    }
    for node in extra_json_ld {
        markup.push_str(&format!(
            "<script type=\"application/ld+json\">{}</script>",
            node
        ));
    }
    markup
}

fn inject_head_markup(document_html: String, head_markup: &str) -> String {
    if head_markup.is_empty() {
        return document_html;
    }

    if let Some(index) = document_html.find("</head>") {
        let mut html = document_html;
        html.insert_str(index, head_markup);
        return html;
    }

    if let Some(index) = document_html.find("<body") {
        let mut html = document_html;
        html.insert_str(index, &format!("<head>{head_markup}</head>"));
        return html;
    }

    format!("<head>{head_markup}</head>{document_html}")
}

fn escape_html_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
