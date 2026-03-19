use super::*;
use davenda_template::{
    AttributeNode, DocumentRenderRequest, ElementNode, FragmentRenderRequest, Node, RenderModel,
    RenderValue, TemplateDefinition, TemplateKind, TemplateModelError, TemplateName,
    TemplateNamespace, TemplateRuntime, TemplateSelector, TrustedHtml,
};

pub(super) fn template_selector(template: &str) -> Result<TemplateSelector, TemplateModelError> {
    Ok(TemplateSelector::new(TemplateName::new(
        template.to_string(),
    )?))
}

impl RuntimePlan {
    pub(super) fn render_fragment_content(
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

    pub(super) fn render_document_shell(
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

    pub(super) fn synthetic_template_runtime(
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
