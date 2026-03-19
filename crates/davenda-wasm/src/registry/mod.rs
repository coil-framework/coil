use std::collections::BTreeMap;

use crate::ids::{ContractVersion, ExtensionId, ExtensionPointKind, HandlerId, HttpMethod};
use crate::manifest::InstalledExtension;

mod install;
mod prepare;

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

    pub fn extensions(&self) -> impl Iterator<Item = &InstalledExtension> {
        self.extensions.values()
    }

    pub fn extension(&self, extension_id: &ExtensionId) -> Option<&InstalledExtension> {
        self.extensions.get(extension_id)
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

    pub fn handler_export(
        &self,
        extension_id: &ExtensionId,
        handler_id: &HandlerId,
    ) -> Option<&str> {
        self.extensions
            .get(extension_id)
            .and_then(|extension| extension.manifest().handler(handler_id))
            .map(|handler| handler.export.as_str())
    }
}
