use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerManifest {
    pub id: HandlerId,
    pub export: String,
    pub point: ExtensionPoint,
    pub requested_grants: HostGrantSet,
    pub limits: Option<ResourceLimits>,
}

impl HandlerManifest {
    pub fn new(
        id: HandlerId,
        export: impl Into<String>,
        point: ExtensionPoint,
        requested_grants: HostGrantSet,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            id,
            export: validate_token("export", export.into())?,
            point,
            requested_grants,
            limits: None,
        })
    }

    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    pub fn effective_limits(&self, manifest_defaults: ResourceLimits) -> ResourceLimits {
        self.limits.unwrap_or(manifest_defaults)
    }

    pub(crate) fn validate(&self, manifest_defaults: ResourceLimits) -> Result<(), WasmModelError> {
        let effective_limits = self.effective_limits(manifest_defaults);
        effective_limits.validate()?;

        for grant in self.requested_grants.iter() {
            if !self.point.supports_grant(grant) {
                return Err(WasmModelError::UnsupportedGrantForPoint {
                    handler_id: self.id.to_string(),
                    point: self.point.kind(),
                    grant: grant.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionManifest {
    pub id: ExtensionId,
    pub display_name: String,
    pub version: ContractVersion,
    pub host_api_version: ContractVersion,
    pub default_limits: ResourceLimits,
    pub handlers: Vec<HandlerManifest>,
}

impl ExtensionManifest {
    pub fn new(
        id: ExtensionId,
        display_name: impl Into<String>,
        version: ContractVersion,
        host_api_version: ContractVersion,
        default_limits: ResourceLimits,
        handlers: Vec<HandlerManifest>,
    ) -> Result<Self, WasmModelError> {
        let manifest = Self {
            id,
            display_name: require_non_empty("display_name", display_name.into())?,
            version,
            host_api_version,
            default_limits,
            handlers,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        self.default_limits.validate()?;
        let mut seen = std::collections::BTreeSet::new();
        for handler in &self.handlers {
            if !seen.insert(handler.id.clone()) {
                return Err(WasmModelError::DuplicateHandlerId {
                    handler_id: handler.id.to_string(),
                });
            }

            handler.validate(self.default_limits)?;
        }
        Ok(())
    }

    pub fn handler(&self, id: &HandlerId) -> Option<&HandlerManifest> {
        self.handlers.iter().find(|handler| &handler.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionArtifactSource {
    LocalPath(String),
    RegistryPackage { registry: String, package: String },
    FirstPartyCatalog { package: String },
}

impl ExtensionArtifactSource {
    pub fn local_path(path: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self::LocalPath(require_non_empty(
            "extension_artifact_path",
            path.into(),
        )?))
    }

    pub fn registry_package(
        registry: impl Into<String>,
        package: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self::RegistryPackage {
            registry: require_non_empty("extension_registry", registry.into())?,
            package: validate_token("extension_registry_package", package.into())?,
        })
    }

    pub fn first_party_catalog(package: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self::FirstPartyCatalog {
            package: validate_token("extension_catalog_package", package.into())?,
        })
    }
}
