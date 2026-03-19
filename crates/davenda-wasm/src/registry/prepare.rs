use super::{ExtensionRegistry, RegisteredExtensionHandler};
use crate::error::WasmModelError;
use crate::ids::HttpMethod;
use crate::invocation::{InvocationContext, InvocationPlan};

impl ExtensionRegistry {
    pub fn prepare_page_invocation(
        &self,
        route: &str,
        method: HttpMethod,
        context: InvocationContext,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        self.page_handlers
            .get(&(route.to_string(), method))
            .map(|handler| self.prepare(handler, context))
            .transpose()
    }

    pub fn prepare_api_invocation(
        &self,
        route: &str,
        method: HttpMethod,
        context: InvocationContext,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        self.api_handlers
            .get(&(route.to_string(), method))
            .map(|handler| self.prepare(handler, context))
            .transpose()
    }

    pub fn prepare_job_invocation(
        &self,
        job_name: &str,
        context: InvocationContext,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        self.job_handlers
            .get(job_name)
            .map(|handler| self.prepare(handler, context))
            .transpose()
    }

    pub fn prepare_scheduled_job_invocation(
        &self,
        job_name: &str,
        context: InvocationContext,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        self.scheduled_job_handlers
            .get(job_name)
            .map(|handler| self.prepare(handler, context))
            .transpose()
    }

    pub fn prepare_webhook_invocation(
        &self,
        source: &str,
        event: &str,
        context: InvocationContext,
    ) -> Result<Option<InvocationPlan>, WasmModelError> {
        self.webhook_handlers
            .get(&(source.to_string(), event.to_string()))
            .map(|handler| self.prepare(handler, context))
            .transpose()
    }

    pub fn prepare_admin_widget_invocations(
        &self,
        slot: &str,
        context: InvocationContext,
    ) -> Result<Vec<InvocationPlan>, WasmModelError> {
        self.prepare_many(self.admin_widget_handlers(slot), context)
    }

    pub fn prepare_render_hook_invocations(
        &self,
        slot: &str,
        context: InvocationContext,
    ) -> Result<Vec<InvocationPlan>, WasmModelError> {
        self.prepare_many(self.render_hook_handlers(slot), context)
    }

    fn prepare(
        &self,
        handler: &RegisteredExtensionHandler,
        context: InvocationContext,
    ) -> Result<InvocationPlan, WasmModelError> {
        let extension = self
            .extensions
            .get(&handler.extension_id)
            .expect("registered handlers always belong to an installed extension");
        extension.prepare_invocation(&handler.handler_id, context)
    }

    fn prepare_many(
        &self,
        handlers: &[RegisteredExtensionHandler],
        context: InvocationContext,
    ) -> Result<Vec<InvocationPlan>, WasmModelError> {
        let mut plans = Vec::new();
        for handler in handlers {
            plans.push(self.prepare(handler, context.clone())?);
        }
        Ok(plans)
    }
}
