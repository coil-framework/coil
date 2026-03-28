use std::collections::BTreeMap;

use crate::SeoError;
use crate::validation::{
    escape_json, require_non_empty, validate_absolute_url, validate_property_name,
};

#[derive(Debug, Clone, PartialEq)]
pub enum JsonLdValue {
    String(String),
    Number(f64),
    Bool(bool),
    Node(JsonLdNode),
    List(Vec<JsonLdValue>),
}

impl JsonLdValue {
    fn render(&self) -> String {
        match self {
            Self::String(value) => format!("\"{}\"", escape_json(value)),
            Self::Number(value) => value.to_string(),
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonLdNode {
    schema_type: String,
    properties: BTreeMap<String, JsonLdValue>,
}

impl JsonLdNode {
    pub fn new(schema_type: impl Into<String>) -> Result<Self, SeoError> {
        Ok(Self {
            schema_type: require_non_empty("schema_type", schema_type.into())?,
            properties: BTreeMap::new(),
        })
    }

    pub fn set_string(
        mut self,
        property: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(
            property,
            JsonLdValue::String(require_non_empty("json_ld_string", value.into())?),
        );
        Ok(self)
    }

    pub fn set_number(mut self, property: impl Into<String>, value: f64) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Number(value));
        Ok(self)
    }

    pub fn set_bool(mut self, property: impl Into<String>, value: bool) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Bool(value));
        Ok(self)
    }

    pub fn set_node(
        mut self,
        property: impl Into<String>,
        node: JsonLdNode,
    ) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
        }
        self.properties.insert(property, JsonLdValue::Node(node));
        Ok(self)
    }

    pub fn set_list(
        mut self,
        property: impl Into<String>,
        values: Vec<JsonLdValue>,
    ) -> Result<Self, SeoError> {
        let property = validate_property_name(property.into())?;
        if self.properties.contains_key(&property) {
            return Err(SeoError::DuplicateJsonLdProperty { property });
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
}

pub fn page_node(
    name: impl Into<String>,
    url: impl Into<String>,
    description: impl Into<String>,
) -> Result<JsonLdNode, SeoError> {
    JsonLdNode::new("WebPage")?
        .set_string("name", name.into())?
        .set_string("url", validate_absolute_url("url", url.into())?)?
        .set_string("description", description.into())
}

pub fn product_node(
    name: impl Into<String>,
    url: impl Into<String>,
    price: f64,
    currency: impl Into<String>,
    availability: impl Into<String>,
) -> Result<JsonLdNode, SeoError> {
    let offer = JsonLdNode::new("Offer")?
        .set_number("price", price)?
        .set_string("priceCurrency", currency.into())?
        .set_string("availability", availability.into())?;

    JsonLdNode::new("Product")?
        .set_string("name", name.into())?
        .set_string("url", validate_absolute_url("url", url.into())?)?
        .set_node("offers", offer)
}

pub fn event_node(
    name: impl Into<String>,
    url: impl Into<String>,
    start_date_unix: i64,
    location_name: impl Into<String>,
) -> Result<JsonLdNode, SeoError> {
    let location = JsonLdNode::new("Place")?.set_string("name", location_name.into())?;

    JsonLdNode::new("Event")?
        .set_string("name", name.into())?
        .set_string("url", validate_absolute_url("url", url.into())?)?
        .set_number("startDateUnix", start_date_unix as f64)?
        .set_node("location", location)
}
