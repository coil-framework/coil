use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
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
        configured: &std::collections::BTreeMap<String, ExtensionConfigValue>,
    ) -> Result<std::collections::BTreeMap<String, ExtensionConfigValue>, WasmModelError> {
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

        let mut effective = std::collections::BTreeMap::new();
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
