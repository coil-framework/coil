use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledHandler {
    pub handler_id: HandlerId,
    pub granted_capabilities: HostGrantSet,
    pub effective_limits: ResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerInstallation {
    pub handler_id: HandlerId,
    pub granted_capabilities: HostGrantSet,
    pub limit_override: Option<ResourceLimits>,
}

impl HandlerInstallation {
    pub fn new(handler_id: HandlerId, granted_capabilities: HostGrantSet) -> Self {
        Self {
            handler_id,
            granted_capabilities,
            limit_override: None,
        }
    }

    pub fn with_limit_override(mut self, limits: ResourceLimits) -> Self {
        self.limit_override = Some(limits);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionInstallation {
    pub customer_app_id: String,
    pub handlers: Vec<HandlerInstallation>,
}

impl ExtensionInstallation {
    pub fn new(
        customer_app_id: impl Into<String>,
        handlers: Vec<HandlerInstallation>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            customer_app_id: validate_token("customer_app_id", customer_app_id.into())?,
            handlers,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtension {
    pub(crate) manifest: ExtensionManifest,
    pub(crate) customer_app_id: String,
    pub(crate) config: std::collections::BTreeMap<String, ExtensionConfigValue>,
    pub(crate) handlers: std::collections::BTreeMap<HandlerId, InstalledHandler>,
    pub(crate) artifact: Option<InstalledArtifact>,
}

impl InstalledExtension {
    pub fn install(
        manifest: ExtensionManifest,
        installation: ExtensionInstallation,
    ) -> Result<Self, WasmModelError> {
        manifest.validate()?;

        let mut handlers = std::collections::BTreeMap::new();
        for configured_handler in installation.handlers {
            let manifest_handler = manifest
                .handler(&configured_handler.handler_id)
                .ok_or_else(|| WasmModelError::HandlerNotFound {
                    handler_id: configured_handler.handler_id.to_string(),
                })?;

            if handlers.contains_key(&configured_handler.handler_id) {
                return Err(WasmModelError::DuplicateInstalledHandler {
                    handler_id: configured_handler.handler_id.to_string(),
                });
            }

            if !configured_handler
                .granted_capabilities
                .is_subset_of(&manifest_handler.requested_grants)
            {
                let offending = configured_handler
                    .granted_capabilities
                    .iter()
                    .find(|grant| !manifest_handler.requested_grants.contains(grant))
                    .expect("subset failure has an offending grant")
                    .clone();

                return Err(WasmModelError::GrantNotDeclared {
                    handler_id: configured_handler.handler_id.to_string(),
                    grant: offending,
                });
            }

            let declared_limits = manifest_handler.effective_limits(manifest.default_limits);
            let effective_limits = configured_handler.limit_override.unwrap_or(declared_limits);
            effective_limits.validate()?;
            effective_limits
                .ensure_no_looser_than(&declared_limits, &configured_handler.handler_id)?;

            handlers.insert(
                configured_handler.handler_id.clone(),
                InstalledHandler {
                    handler_id: configured_handler.handler_id,
                    granted_capabilities: configured_handler.granted_capabilities,
                    effective_limits,
                },
            );
        }

        Ok(Self {
            manifest,
            customer_app_id: installation.customer_app_id,
            config: std::collections::BTreeMap::new(),
            handlers,
            artifact: None,
        })
    }

    pub fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    pub fn customer_app_id(&self) -> &str {
        &self.customer_app_id
    }

    pub fn installed_handler_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn config(&self) -> &std::collections::BTreeMap<String, ExtensionConfigValue> {
        &self.config
    }

    pub fn artifact(&self) -> Option<&InstalledArtifact> {
        self.artifact.as_ref()
    }

    pub fn prepare_invocation(
        &self,
        handler_id: &HandlerId,
        mut context: InvocationContext,
    ) -> Result<InvocationPlan, WasmModelError> {
        context.extension_config = self.config.clone();
        context.validate()?;

        let manifest_handler =
            self.manifest
                .handler(handler_id)
                .ok_or_else(|| WasmModelError::HandlerNotFound {
                    handler_id: handler_id.to_string(),
                })?;
        let installed_handler =
            self.handlers
                .get(handler_id)
                .ok_or_else(|| WasmModelError::HandlerNotFound {
                    handler_id: handler_id.to_string(),
                })?;

        let actual_point = context.input.kind();
        let expected_point = manifest_handler.point.kind();
        if actual_point != expected_point {
            return Err(WasmModelError::InvocationPointMismatch {
                handler_id: handler_id.to_string(),
                expected: expected_point,
                actual: actual_point,
            });
        }

        crate::validation::validate_invocation_target(
            handler_id,
            &manifest_handler.point,
            &context.input,
        )?;

        Ok(InvocationPlan {
            extension_id: self.manifest.id.clone(),
            handler_id: handler_id.clone(),
            point: manifest_handler.point.kind(),
            customer_app_id: self.customer_app_id.clone(),
            granted_capabilities: installed_handler.granted_capabilities.clone(),
            limits: installed_handler.effective_limits,
            context,
        })
    }
}
