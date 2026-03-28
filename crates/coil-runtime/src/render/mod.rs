use super::*;
use coil_i18n::I18nError;
use coil_seo::SeoError;
use coil_template::{DocumentRenderRequest, TemplateKind, TemplateModelError};
use thiserror::Error;

mod model;
mod seo;
mod templates;

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
        let selector = templates::template_selector(&page.template)?;
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
        let selector = templates::template_selector(&fragment.template)?;
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
}
