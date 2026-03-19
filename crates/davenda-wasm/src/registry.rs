use std::collections::BTreeMap;

use crate::error::WasmModelError;
use crate::ids::{ContractVersion, ExtensionId, ExtensionPointKind, HandlerId, HttpMethod};
use crate::invocation::{InvocationContext, InvocationPlan};
use crate::manifest::InstalledExtension;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredExtensionHandler {
    pub extension_id: ExtensionId,
    pub handler_id: HandlerId,
    pub point: ExtensionPointKind,
    pub surface: String,
    pub selector: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionRegistry {
    host_api_version: ContractVersion,
    customer_app_id: Option<String>,
    extensions: BTreeMap<ExtensionId, InstalledExtension>,
    registered_handlers: Vec<RegisteredExtensionHandler>,
    page_handlers: BTreeMap<(String, HttpMethod), RegisteredExtensionHandler>,
    api_handlers: BTreeMap<(String, HttpMethod), RegisteredExtensionHandler>,
    job_handlers: BTreeMap<String, RegisteredExtensionHandler>,
    scheduled_job_handlers: BTreeMap<String, RegisteredExtensionHandler>,
    webhook_handlers: BTreeMap<(String, String), RegisteredExtensionHandler>,
    admin_widget_handlers: BTreeMap<String, Vec<RegisteredExtensionHandler>>,
    render_hook_handlers: BTreeMap<String, Vec<RegisteredExtensionHandler>>,
}

impl ExtensionRegistry {
    pub fn new(host_api_version: ContractVersion) -> Self {
        Self {
            host_api_version,
            customer_app_id: None,
            extensions: BTreeMap::new(),
            registered_handlers: Vec::new(),
            page_handlers: BTreeMap::new(),
            api_handlers: BTreeMap::new(),
            job_handlers: BTreeMap::new(),
            scheduled_job_handlers: BTreeMap::new(),
            webhook_handlers: BTreeMap::new(),
            admin_widget_handlers: BTreeMap::new(),
            render_hook_handlers: BTreeMap::new(),
        }
    }

    pub fn customer_app_id(&self) -> Option<&str> {
        self.customer_app_id.as_deref()
    }

    pub fn host_api_version(&self) -> ContractVersion {
        self.host_api_version
    }

    pub fn install(&mut self, extension: InstalledExtension) -> Result<(), WasmModelError> {
        let extension_id = extension.manifest.id.clone();
        if self.extensions.contains_key(&extension_id) {
            return Err(WasmModelError::DuplicateInstalledExtension {
                extension_id: extension_id.to_string(),
            });
        }

        if !extension
            .manifest
            .host_api_version
            .is_compatible_with(self.host_api_version)
        {
            return Err(WasmModelError::HostApiVersionMismatch {
                extension_id: extension_id.to_string(),
                expected: self.host_api_version,
                actual: extension.manifest.host_api_version,
            });
        }

        if let Some(expected_app_id) = &self.customer_app_id {
            if expected_app_id != &extension.customer_app_id {
                return Err(WasmModelError::MixedCustomerAppInstallation {
                    extension_id: extension_id.to_string(),
                    expected: expected_app_id.clone(),
                    actual: extension.customer_app_id.clone(),
                });
            }
        } else {
            self.customer_app_id = Some(extension.customer_app_id.clone());
        }

        for handler_id in extension.handlers.keys() {
            let manifest_handler = extension
                .manifest
                .handler(handler_id)
                .expect("installed handlers must exist in the manifest");

            match &manifest_handler.point {
                crate::points::ExtensionPoint::Page(page) => {
                    for method in &page.methods {
                        let key = (page.route.clone(), *method);
                        let selector = format!("{method} {}", page.route);
                        let binding = RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: ExtensionPointKind::Page,
                            surface: page.route.clone(),
                            selector: selector.clone(),
                        };
                        crate::validation::register_unique_target(
                            &mut self.page_handlers,
                            key,
                            binding,
                            selector,
                            ExtensionPointKind::Page,
                        )?;
                        self.registered_handlers.push(RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: ExtensionPointKind::Page,
                            surface: page.route.clone(),
                            selector: format!("{method} {}", page.route),
                        });
                    }
                }
                crate::points::ExtensionPoint::Api(api) => {
                    for method in &api.methods {
                        let key = (api.route.clone(), *method);
                        let selector = format!("{method} {}", api.route);
                        let binding = RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: ExtensionPointKind::Api,
                            surface: api.route.clone(),
                            selector: selector.clone(),
                        };
                        crate::validation::register_unique_target(
                            &mut self.api_handlers,
                            key,
                            binding,
                            selector,
                            ExtensionPointKind::Api,
                        )?;
                        self.registered_handlers.push(RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: ExtensionPointKind::Api,
                            surface: api.route.clone(),
                            selector: format!("{method} {}", api.route),
                        });
                    }
                }
                crate::points::ExtensionPoint::Job(job) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::Job,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    };
                    crate::validation::register_unique_target(
                        &mut self.job_handlers,
                        job.job_name.clone(),
                        binding,
                        job.job_name.clone(),
                        ExtensionPointKind::Job,
                    )?;
                    self.registered_handlers.push(RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::Job,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    });
                }
                crate::points::ExtensionPoint::ScheduledJob(job) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::ScheduledJob,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    };
                    crate::validation::register_unique_target(
                        &mut self.scheduled_job_handlers,
                        job.job_name.clone(),
                        binding,
                        job.job_name.clone(),
                        ExtensionPointKind::ScheduledJob,
                    )?;
                    self.registered_handlers.push(RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::ScheduledJob,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    });
                }
                crate::points::ExtensionPoint::Webhook(webhook) => {
                    let selector = format!("{}/{}", webhook.source, webhook.event);
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::Webhook,
                        surface: webhook.source.clone(),
                        selector: selector.clone(),
                    };
                    crate::validation::register_unique_target(
                        &mut self.webhook_handlers,
                        (webhook.source.clone(), webhook.event.clone()),
                        binding,
                        selector,
                        ExtensionPointKind::Webhook,
                    )?;
                    self.registered_handlers.push(RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::Webhook,
                        surface: webhook.source.clone(),
                        selector: format!("{}/{}", webhook.source, webhook.event),
                    });
                }
                crate::points::ExtensionPoint::AdminWidget(widget) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::AdminWidget,
                        surface: widget.slot.clone(),
                        selector: widget.slot.clone(),
                    };
                    self.registered_handlers.push(binding.clone());
                    self.admin_widget_handlers
                        .entry(widget.slot.clone())
                        .or_default()
                        .push(binding);
                }
                crate::points::ExtensionPoint::RenderHook(hook) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: ExtensionPointKind::RenderHook,
                        surface: hook.slot.clone(),
                        selector: hook.slot.clone(),
                    };
                    self.registered_handlers.push(binding.clone());
                    self.render_hook_handlers
                        .entry(hook.slot.clone())
                        .or_default()
                        .push(binding);
                }
            }
        }

        self.extensions.insert(extension_id, extension);
        Ok(())
    }

    pub fn extensions(&self) -> impl Iterator<Item = &InstalledExtension> {
        self.extensions.values()
    }

    pub fn registered_handlers(&self) -> &[RegisteredExtensionHandler] {
        &self.registered_handlers
    }

    pub fn admin_widget_handlers(&self, slot: &str) -> &[RegisteredExtensionHandler] {
        self.admin_widget_handlers
            .get(slot)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn render_hook_handlers(&self, slot: &str) -> &[RegisteredExtensionHandler] {
        self.render_hook_handlers
            .get(slot)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

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
