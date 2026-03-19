use super::{ExtensionRegistry, RegisteredExtensionHandler};
use crate::error::WasmModelError;
use crate::manifest::InstalledExtension;
use crate::points::ExtensionPoint;

impl ExtensionRegistry {
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
                ExtensionPoint::Page(page) => {
                    for method in &page.methods {
                        let key = (page.route.clone(), *method);
                        let selector = format!("{method} {}", page.route);
                        let binding = RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: crate::ExtensionPointKind::Page,
                            surface: page.route.clone(),
                            selector: selector.clone(),
                        };
                        crate::validation::register_unique_target(
                            &mut self.page_handlers,
                            key,
                            binding,
                            selector,
                            crate::ExtensionPointKind::Page,
                        )?;
                        self.registered_handlers.push(RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: crate::ExtensionPointKind::Page,
                            surface: page.route.clone(),
                            selector: format!("{method} {}", page.route),
                        });
                    }
                }
                ExtensionPoint::Api(api) => {
                    for method in &api.methods {
                        let key = (api.route.clone(), *method);
                        let selector = format!("{method} {}", api.route);
                        let binding = RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: crate::ExtensionPointKind::Api,
                            surface: api.route.clone(),
                            selector: selector.clone(),
                        };
                        crate::validation::register_unique_target(
                            &mut self.api_handlers,
                            key,
                            binding,
                            selector,
                            crate::ExtensionPointKind::Api,
                        )?;
                        self.registered_handlers.push(RegisteredExtensionHandler {
                            extension_id: extension_id.clone(),
                            handler_id: handler_id.clone(),
                            point: crate::ExtensionPointKind::Api,
                            surface: api.route.clone(),
                            selector: format!("{method} {}", api.route),
                        });
                    }
                }
                ExtensionPoint::Job(job) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::Job,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    };
                    crate::validation::register_unique_target(
                        &mut self.job_handlers,
                        job.job_name.clone(),
                        binding,
                        job.job_name.clone(),
                        crate::ExtensionPointKind::Job,
                    )?;
                    self.registered_handlers.push(RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::Job,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    });
                }
                ExtensionPoint::ScheduledJob(job) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::ScheduledJob,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    };
                    crate::validation::register_unique_target(
                        &mut self.scheduled_job_handlers,
                        job.job_name.clone(),
                        binding,
                        job.job_name.clone(),
                        crate::ExtensionPointKind::ScheduledJob,
                    )?;
                    self.registered_handlers.push(RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::ScheduledJob,
                        surface: job.job_name.clone(),
                        selector: job.job_name.clone(),
                    });
                }
                ExtensionPoint::Webhook(webhook) => {
                    let selector = format!("{}/{}", webhook.source, webhook.event);
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::Webhook,
                        surface: webhook.source.clone(),
                        selector: selector.clone(),
                    };
                    crate::validation::register_unique_target(
                        &mut self.webhook_handlers,
                        (webhook.source.clone(), webhook.event.clone()),
                        binding,
                        selector,
                        crate::ExtensionPointKind::Webhook,
                    )?;
                    self.registered_handlers.push(RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::Webhook,
                        surface: webhook.source.clone(),
                        selector: format!("{}/{}", webhook.source, webhook.event),
                    });
                }
                ExtensionPoint::AdminWidget(widget) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::AdminWidget,
                        surface: widget.slot.clone(),
                        selector: widget.slot.clone(),
                    };
                    self.registered_handlers.push(binding.clone());
                    self.admin_widget_handlers
                        .entry(widget.slot.clone())
                        .or_default()
                        .push(binding);
                }
                ExtensionPoint::RenderHook(hook) => {
                    let binding = RegisteredExtensionHandler {
                        extension_id: extension_id.clone(),
                        handler_id: handler_id.clone(),
                        point: crate::ExtensionPointKind::RenderHook,
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
}
