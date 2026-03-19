use super::*;

impl RuntimeHostServiceExecutor {
    pub(super) fn execute_render(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
        request: &RenderServiceRequest,
    ) -> Result<HostServiceExecution, WasmModelError> {
        let fragment = self.render_fragment(request, context)?;
        Ok(self.host_service_execution(
            call,
            HostServiceResult::Render(RenderServiceExecution {
                request: request.clone(),
                fragment,
            }),
        ))
    }

    fn render_fragment(
        &self,
        request: &RenderServiceRequest,
        context: &InvocationContext,
    ) -> Result<String, WasmModelError> {
        let slot = match request {
            RenderServiceRequest::Fragment { slot } => slot,
        };
        let fragment_name = TemplateName::new(format!("wasm-host-{slot}"))
            .map_err(|error| runtime_executor_error(context, error))?;
        let definition = TemplateDefinition::fragment(
            self.plan.template.customer_app_namespace.clone(),
            fragment_name.clone(),
            vec![Node::Element(
                ElementNode::new(
                    "div",
                    vec![Node::static_text(format!(
                        "host-render:{}:{}",
                        context.customer_app.app_id, slot
                    ))],
                )
                .map_err(|error| runtime_executor_error(context, error))?
                .with_attribute(
                    AttributeNode::static_value("data-slot", slot)
                        .map_err(|error| runtime_executor_error(context, error))?,
                )
                .with_attribute(
                    AttributeNode::static_value("data-app", context.customer_app.app_id.clone())
                        .map_err(|error| runtime_executor_error(context, error))?,
                )
                .with_attribute(
                    AttributeNode::static_value(
                        "data-locale",
                        context
                            .customer_app
                            .locale
                            .clone()
                            .unwrap_or_else(|| self.plan.config.i18n.default_locale.clone()),
                    )
                    .map_err(|error| runtime_executor_error(context, error))?,
                ),
            )],
        );
        let mut registry = self.plan.template.registry.clone();
        registry
            .register(definition)
            .map_err(|error| runtime_executor_error(context, error))?;
        let runtime = TemplateRuntime::new(registry);
        let selector = TemplateSelector::new(fragment_name);
        let model = RenderModel::new()
            .with_value(
                "customer_app",
                RenderValue::text(context.customer_app.app_id.clone()),
            )
            .map_err(|error| runtime_executor_error(context, error))?
            .with_value("slot", RenderValue::text(slot.clone()))
            .map_err(|error| runtime_executor_error(context, error))?
            .with_value(
                "locale",
                RenderValue::text(
                    context
                        .customer_app
                        .locale
                        .clone()
                        .unwrap_or_else(|| self.plan.config.i18n.default_locale.clone()),
                ),
            )
            .map_err(|error| runtime_executor_error(context, error))?;

        runtime
            .render_fragment(
                &[self.plan.template.customer_app_namespace.clone()],
                FragmentRenderRequest::new(selector, model),
            )
            .map(|output| output.html)
            .map_err(|error| runtime_executor_error(context, error))
    }
}
