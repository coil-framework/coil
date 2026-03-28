use std::collections::BTreeMap;
use std::fmt;

use crate::error::WasmModelError;
use crate::validation::{require_non_empty, validate_token};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RobotsDirective {
    Index,
    NoIndex,
    Follow,
    NoFollow,
    NoArchive,
}

impl fmt::Display for RobotsDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index => f.write_str("index"),
            Self::NoIndex => f.write_str("noindex"),
            Self::Follow => f.write_str("follow"),
            Self::NoFollow => f.write_str("nofollow"),
            Self::NoArchive => f.write_str("noarchive"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonLdValue {
    String(String),
    Number(String),
    Bool(bool),
    Node(JsonLdNode),
    List(Vec<JsonLdValue>),
}

impl JsonLdValue {
    fn render(&self) -> String {
        match self {
            Self::String(value) => format!("\"{}\"", escape_json(value)),
            Self::Number(value) => value.clone(),
            Self::Bool(value) => value.to_string(),
            Self::Node(node) => node.render(),
            Self::List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(JsonLdValue::render)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }

    pub(crate) fn validate(&self) -> Result<(), WasmModelError> {
        match self {
            Self::String(value) => {
                let _ = require_non_empty("json_ld_string", value.clone())?;
            }
            Self::Number(value) => {
                if value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .is_none()
                {
                    return Err(WasmModelError::InvalidJsonLdNumber {
                        property: "json_ld_number".to_string(),
                        value: value.clone(),
                    });
                }
            }
            Self::Bool(_) => {}
            Self::Node(node) => node.validate()?,
            Self::List(values) => {
                for value in values {
                    value.validate()?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonLdNode {
    schema_type: String,
    properties: BTreeMap<String, JsonLdValue>,
}

impl JsonLdNode {
    pub fn new(schema_type: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            schema_type: validate_token("schema_type", schema_type.into())?,
            properties: BTreeMap::new(),
        })
    }

    pub fn set_string(
        mut self,
        property: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(
            property,
            JsonLdValue::String(require_non_empty("json_ld_string", value.into())?),
        );
        Ok(self)
    }

    pub fn set_number(
        mut self,
        property: impl Into<String>,
        value: f64,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        if !value.is_finite() {
            return Err(WasmModelError::InvalidJsonLdNumber {
                property,
                value: value.to_string(),
            });
        }
        self.properties
            .insert(property, JsonLdValue::Number(value.to_string()));
        Ok(self)
    }

    pub fn set_bool(
        mut self,
        property: impl Into<String>,
        value: bool,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Bool(value));
        Ok(self)
    }

    pub fn set_node(
        mut self,
        property: impl Into<String>,
        node: JsonLdNode,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Node(node));
        Ok(self)
    }

    pub fn set_list(
        mut self,
        property: impl Into<String>,
        values: Vec<JsonLdValue>,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::List(values));
        Ok(self)
    }

    pub fn render(&self) -> String {
        let mut segments = vec![format!("\"@type\":\"{}\"", escape_json(&self.schema_type))];
        for (property, value) in &self.properties {
            segments.push(format!("\"{}\":{}", escape_json(property), value.render()));
        }
        format!("{{{}}}", segments.join(","))
    }

    pub(crate) fn schema_type(&self) -> &str {
        &self.schema_type
    }

    pub(crate) fn properties(&self) -> &BTreeMap<String, JsonLdValue> {
        &self.properties
    }

    pub(crate) fn insert_value(
        mut self,
        property: impl Into<String>,
        value: JsonLdValue,
    ) -> Result<Self, WasmModelError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(WasmModelError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, value);
        Ok(self)
    }

    pub(crate) fn validate(&self) -> Result<(), WasmModelError> {
        let _ = validate_token("schema_type", self.schema_type.clone())?;
        for (property, value) in &self.properties {
            let _ = validate_property_name(property.clone())?;
            value.validate()?;
        }
        Ok(())
    }
}

fn validate_property_name(value: String) -> Result<String, WasmModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '@' | '_' | '-'))
    {
        Err(WasmModelError::InvalidJsonLdProperty {
            property: trimmed.to_string(),
        })
    } else {
        Ok(trimmed.to_string())
    }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
