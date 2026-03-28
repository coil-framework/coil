use super::*;
use coil_template::{
    AttributeNode, DocumentRenderRequest, ElementNode, FragmentRenderRequest, Node, RenderModel,
    RenderValue, TemplateDefinition, TemplateKind, TemplateModelError, TemplateName,
    TemplateNamespace, TemplateRuntime, TemplateSelector, TemplateSourceParser, TrustedHtml,
};
use std::path::PathBuf;

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

pub(super) fn load_customer_templates_from_roots(
    template_roots: &[PathBuf],
    namespace: TemplateNamespace,
) -> Result<Vec<TemplateDefinition>, TemplateModelError> {
    let parser = TemplateSourceParser::new();
    let mut templates = Vec::new();
    for root in template_roots {
        let template_dir = root.join("templates");
        if template_dir.exists() {
            templates.extend(parser.load_directory(&template_dir, namespace.clone())?);
        }
    }
    Ok(templates)
}
