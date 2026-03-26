use super::*;
#[cfg(test)]
use crate::runtime::escape_html_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedHtml(String);

impl TrustedHtml {
    pub fn new(value: impl Into<String>) -> Result<Self, TemplateModelError> {
        Ok(Self(require_non_empty("trusted_html", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderValue {
    Text(String),
    TrustedHtml(TrustedHtml),
    Bool(bool),
    List(Vec<RenderModel>),
    Object(RenderModel),
}

impl RenderValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn trusted_html(value: TrustedHtml) -> Self {
        Self::TrustedHtml(value)
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub fn list(value: Vec<RenderModel>) -> Self {
        Self::List(value)
    }

    pub fn object(value: RenderModel) -> Self {
        Self::Object(value)
    }

    pub(crate) fn as_text(&self, key: &str) -> Result<&str, TemplateModelError> {
        match self {
            Self::Text(value) => Ok(value),
            Self::TrustedHtml(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
            Self::Bool(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
            Self::List(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
            Self::Object(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
        }
    }

    pub(crate) fn as_bool(&self, key: &str) -> Result<bool, TemplateModelError> {
        match self {
            Self::Bool(value) => Ok(*value),
            Self::Text(_) | Self::TrustedHtml(_) | Self::List(_) | Self::Object(_) => {
                Err(TemplateModelError::ValueTypeMismatch {
                    key: key.to_string(),
                    expected: "bool",
                })
            }
        }
    }

    pub(crate) fn as_list(&self, key: &str) -> Result<&[RenderModel], TemplateModelError> {
        match self {
            Self::List(value) => Ok(value.as_slice()),
            Self::Text(_) | Self::TrustedHtml(_) | Self::Bool(_) | Self::Object(_) => {
                Err(TemplateModelError::ValueTypeMismatch {
                    key: key.to_string(),
                    expected: "list",
                })
            }
        }
    }

    pub(crate) fn as_object(&self, key: &str) -> Result<&RenderModel, TemplateModelError> {
        match self {
            Self::Object(value) => Ok(value),
            Self::Text(_) | Self::TrustedHtml(_) | Self::Bool(_) | Self::List(_) => {
                Err(TemplateModelError::ValueTypeMismatch {
                    key: key.to_string(),
                    expected: "object",
                })
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn render_html(&self) -> String {
        match self {
            Self::Text(value) => escape_html_text(value),
            Self::TrustedHtml(value) => value.as_str().to_string(),
            Self::Bool(value) => value.to_string(),
            Self::List(_) => String::new(),
            Self::Object(_) => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderModel {
    values: BTreeMap<String, RenderValue>,
    asset_paths: BTreeMap<String, String>,
}

impl RenderModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_value(
        mut self,
        key: impl Into<String>,
        value: RenderValue,
    ) -> Result<Self, TemplateModelError> {
        let key = validate_token("render_key", key.into())?;
        self.values.insert(key, value);
        Ok(self)
    }

    pub fn with_bool(
        self,
        key: impl Into<String>,
        value: bool,
    ) -> Result<Self, TemplateModelError> {
        self.with_value(key, RenderValue::bool(value))
    }

    pub fn with_list(
        self,
        key: impl Into<String>,
        value: Vec<RenderModel>,
    ) -> Result<Self, TemplateModelError> {
        self.with_value(key, RenderValue::list(value))
    }

    pub fn with_object(
        self,
        key: impl Into<String>,
        value: RenderModel,
    ) -> Result<Self, TemplateModelError> {
        self.with_value(key, RenderValue::object(value))
    }

    pub fn with_asset_path(
        mut self,
        logical_path: impl Into<String>,
        public_url: impl Into<String>,
    ) -> Result<Self, TemplateModelError> {
        let logical_path = validate_token("asset_logical_path", logical_path.into())?;
        let public_url = require_non_empty("asset_public_url", public_url.into())?;
        self.asset_paths.insert(logical_path, public_url);
        Ok(self)
    }

    pub(crate) fn get(&self, key: &str) -> Option<&RenderValue> {
        if let Some(value) = self.values.get(key) {
            return Some(value);
        }

        let (head, tail) = key.split_once('.')?;
        let value = self.values.get(head)?;
        value.as_object(head).ok()?.get(tail)
    }

    pub(crate) fn get_path(&self, path: &str) -> Option<&RenderValue> {
        self.get(path)
    }

    pub(crate) fn get_asset_path(&self, logical_path: &str) -> Option<&str> {
        self.asset_paths.get(logical_path).map(String::as_str)
    }

    pub(crate) fn merged_with(&self, overlay: &RenderModel) -> RenderModel {
        let mut values = self.values.clone();
        values.extend(overlay.values.clone());
        let mut asset_paths = self.asset_paths.clone();
        asset_paths.extend(overlay.asset_paths.clone());
        RenderModel { values, asset_paths }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub html: String,
}
