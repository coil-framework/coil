use std::collections::BTreeMap;

use crate::artifact::InstalledArtifact;
use crate::error::WasmModelError;
use crate::grants::{HostGrantSet, ResourceLimits};
use crate::ids::{ContractVersion, ExtensionId, HandlerId};
use crate::invocation::{InvocationContext, InvocationPlan};
use crate::points::ExtensionPoint;
use crate::validation::{require_non_empty, validate_sha256, validate_token};

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

    fn validate(&self, manifest_defaults: ResourceLimits) -> Result<(), WasmModelError> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionConfigValueType {
    String,
    Integer,
    Boolean,
}

impl std::fmt::Display for ExtensionConfigValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String => f.write_str("string"),
            Self::Integer => f.write_str("integer"),
            Self::Boolean => f.write_str("boolean"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionConfigValue {
    String(String),
    Integer(i64),
    Boolean(bool),
}

impl ExtensionConfigValue {
    pub fn value_type(&self) -> ExtensionConfigValueType {
        match self {
            Self::String(_) => ExtensionConfigValueType::String,
            Self::Integer(_) => ExtensionConfigValueType::Integer,
            Self::Boolean(_) => ExtensionConfigValueType::Boolean,
        }
    }

    pub(crate) fn validate_for_key(&self, key: &str) -> Result<(), WasmModelError> {
        if let Self::String(value) = self {
            if value.trim().is_empty() {
                return Err(WasmModelError::InvalidConfigValue {
                    key: key.to_string(),
                    reason: "string values must not be empty".to_string(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionConfigField {
    pub key: String,
    pub value_type: ExtensionConfigValueType,
    pub required: bool,
    pub default: Option<ExtensionConfigValue>,
}

impl ExtensionConfigField {
    pub fn new(
        key: impl Into<String>,
        value_type: ExtensionConfigValueType,
        required: bool,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            key: validate_token("extension_config_key", key.into())?,
            value_type,
            required,
            default: None,
        })
    }

    pub fn required(
        key: impl Into<String>,
        value_type: ExtensionConfigValueType,
    ) -> Result<Self, WasmModelError> {
        Self::new(key, value_type, true)
    }

    pub fn optional(
        key: impl Into<String>,
        value_type: ExtensionConfigValueType,
    ) -> Result<Self, WasmModelError> {
        Self::new(key, value_type, false)
    }

    pub fn with_default(mut self, value: ExtensionConfigValue) -> Result<Self, WasmModelError> {
        if value.value_type() != self.value_type {
            return Err(WasmModelError::ConfigTypeMismatch {
                key: self.key.clone(),
                expected: self.value_type,
                actual: value.value_type(),
            });
        }
        value.validate_for_key(&self.key)?;
        self.default = Some(value);
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionConfigSchema {
    pub version: u32,
    pub fields: Vec<ExtensionConfigField>,
}

impl ExtensionConfigSchema {
    pub fn new(version: u32, fields: Vec<ExtensionConfigField>) -> Result<Self, WasmModelError> {
        if version == 0 {
            return Err(WasmModelError::ZeroSchemaVersion {
                field: "extension_config_schema_version",
            });
        }

        let schema = Self { version, fields };
        schema.validate()?;
        Ok(schema)
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        let mut seen = std::collections::BTreeSet::new();
        for field in &self.fields {
            if !seen.insert(field.key.clone()) {
                return Err(WasmModelError::DuplicateConfigField {
                    key: field.key.clone(),
                });
            }

            if let Some(default) = &field.default {
                if default.value_type() != field.value_type {
                    return Err(WasmModelError::ConfigTypeMismatch {
                        key: field.key.clone(),
                        expected: field.value_type,
                        actual: default.value_type(),
                    });
                }
                default.validate_for_key(&field.key)?;
            }
        }
        Ok(())
    }

    pub fn effective_values(
        &self,
        configured: &BTreeMap<String, ExtensionConfigValue>,
    ) -> Result<BTreeMap<String, ExtensionConfigValue>, WasmModelError> {
        self.validate()?;

        for (key, value) in configured {
            let Some(field) = self.fields.iter().find(|field| field.key == *key) else {
                return Err(WasmModelError::UnknownConfigField { key: key.clone() });
            };

            if value.value_type() != field.value_type {
                return Err(WasmModelError::ConfigTypeMismatch {
                    key: key.clone(),
                    expected: field.value_type,
                    actual: value.value_type(),
                });
            }

            value.validate_for_key(key)?;
        }

        let mut effective = BTreeMap::new();
        for field in &self.fields {
            if let Some(value) = configured.get(&field.key) {
                effective.insert(field.key.clone(), value.clone());
            } else if let Some(default) = &field.default {
                effective.insert(field.key.clone(), default.clone());
            } else if field.required {
                return Err(WasmModelError::MissingRequiredConfigField {
                    key: field.key.clone(),
                });
            }
        }

        Ok(effective)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionPackage {
    pub publisher: String,
    pub manifest: ExtensionManifest,
    pub artifact_source: ExtensionArtifactSource,
    pub artifact_sha256: String,
    pub config_schema: ExtensionConfigSchema,
}

impl ExtensionPackage {
    pub fn new(
        publisher: impl Into<String>,
        manifest: ExtensionManifest,
        artifact_source: ExtensionArtifactSource,
        artifact_sha256: impl Into<String>,
        config_schema: ExtensionConfigSchema,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            publisher: require_non_empty("extension_publisher", publisher.into())?,
            manifest,
            artifact_source,
            artifact_sha256: validate_sha256("extension_artifact_sha256", artifact_sha256.into())?,
            config_schema,
        })
    }

    pub fn install(
        &self,
        installation: ExtensionInstallation,
        configured_values: &BTreeMap<String, ExtensionConfigValue>,
    ) -> Result<InstalledExtension, WasmModelError> {
        let mut installed = InstalledExtension::install(self.manifest.clone(), installation)?;
        installed.config = self.config_schema.effective_values(configured_values)?;
        installed.artifact = Some(InstalledArtifact::new(
            self.publisher.clone(),
            self.artifact_source.clone(),
            self.artifact_sha256.clone(),
        )?);
        Ok(installed)
    }

    pub fn id(&self) -> &ExtensionId {
        &self.manifest.id
    }

    pub fn version(&self) -> ContractVersion {
        self.manifest.version
    }
}

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
    pub(crate) config: BTreeMap<String, ExtensionConfigValue>,
    pub(crate) handlers: BTreeMap<HandlerId, InstalledHandler>,
    pub(crate) artifact: Option<InstalledArtifact>,
}

impl InstalledExtension {
    pub fn install(
        manifest: ExtensionManifest,
        installation: ExtensionInstallation,
    ) -> Result<Self, WasmModelError> {
        manifest.validate()?;

        let mut handlers = BTreeMap::new();
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
            config: BTreeMap::new(),
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

    pub fn config(&self) -> &BTreeMap<String, ExtensionConfigValue> {
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
