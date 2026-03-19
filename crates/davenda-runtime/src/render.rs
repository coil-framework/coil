use super::*;
use davenda_template::{
    AttributeNode, DocumentRenderRequest, ElementNode, FragmentRenderRequest, Node, RenderModel,
    RenderValue, TemplateDefinition, TemplateKind, TemplateModelError, TemplateName,
    TemplateNamespace, TemplateRuntime, TemplateSelector, TrustedHtml,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeRenderError {
    #[error(transparent)]
    Template(#[from] TemplateModelError),
}

impl RuntimePlan {
    pub fn render_page_response(
        &self,
        execution: &RequestExecution,
        page: &PageResponse,
    ) -> Result<String, RuntimeRenderError> {
        let selector = template_selector(&page.template)?;
        let namespaces = self.template_namespaces_for_execution(execution);
        let model = self.render_model_for_execution(execution, &page.template, None)?;

        match self.template.runtime.render_document(
            &namespaces,
            DocumentRenderRequest::new(selector.clone(), model.clone()),
        ) {
            Ok(output) => Ok(output.html),
            Err(TemplateModelError::TemplateNotFound { .. })
            | Err(TemplateModelError::TemplateKindMismatch {
                actual: TemplateKind::Fragment,
                ..
            }) => {
                let content =
                    self.render_fragment_content(execution, &namespaces, &selector, model, None)?;
                Ok(self.render_document_shell(execution, &page.template, content)?)
            }
            Err(error) => Err(error.into()),
        }
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
    let path = ElementNode::new("p", vec![Node::value("path")?])?
        .with_attribute(AttributeNode::static_value("class", "davenda-runtime-path")?);
    let template = ElementNode::new("p", vec![Node::value("template_name")?])?
        .with_attribute(AttributeNode::static_value("class", "davenda-runtime-template")?);
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
