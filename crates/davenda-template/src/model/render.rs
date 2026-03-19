use super::*;

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
}

impl RenderValue {
    pub fn text(value: impl Into<String>) -> Self {
        Self::Text(value.into())
    }

    pub fn trusted_html(value: TrustedHtml) -> Self {
        Self::TrustedHtml(value)
    }

    pub(crate) fn as_text(&self, key: &str) -> Result<&str, TemplateModelError> {
        match self {
            Self::Text(value) => Ok(value),
            Self::TrustedHtml(_) => Err(TemplateModelError::ValueTypeMismatch {
                key: key.to_string(),
                expected: "text",
            }),
        }
    }

    pub(crate) fn render_html(&self) -> String {
        match self {
            Self::Text(value) => escape_html_text(value),
            Self::TrustedHtml(value) => value.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderModel {
    values: BTreeMap<String, RenderValue>,
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

    pub(crate) fn get(&self, key: &str) -> Option<&RenderValue> {
        self.values.get(key)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub html: String,
}
