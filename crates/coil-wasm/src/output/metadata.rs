use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::error::WasmModelError;
use crate::validation::{require_non_empty, validate_token};

use super::json_ld::{JsonLdNode, RobotsDirective};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TypedMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub canonical_url: Option<String>,
    pub alternate_urls: BTreeMap<String, String>,
    pub robots: BTreeSet<RobotsDirective>,
    pub json_ld: Vec<JsonLdNode>,
}

impl TypedMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Result<Self, WasmModelError> {
        self.title = Some(require_non_empty("title", title.into())?);
        Ok(self)
    }

    pub fn with_description(
        mut self,
        description: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.description = Some(require_non_empty("description", description.into())?);
        Ok(self)
    }

    pub fn with_canonical_url(
        mut self,
        canonical_url: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.canonical_url = Some(validate_absolute_url(
            "canonical_url",
            canonical_url.into(),
        )?);
        Ok(self)
    }

    pub fn insert_alternate_url(
        mut self,
        locale: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        let locale = validate_token("alternate_url_locale", locale.into())?;
        let url = validate_absolute_url("alternate_url", url.into())?;
        self.alternate_urls.insert(locale, url);
        Ok(self)
    }

    pub fn with_robot_directive(mut self, directive: RobotsDirective) -> Self {
        self.robots.insert(directive);
        self
    }

    pub fn push_json_ld(mut self, node: JsonLdNode) -> Self {
        self.json_ld.push(node);
        self
    }

    pub fn merge_from(&mut self, other: &Self) {
        if other.title.is_some() {
            self.title = other.title.clone();
        }
        if other.description.is_some() {
            self.description = other.description.clone();
        }
        if other.canonical_url.is_some() {
            self.canonical_url = other.canonical_url.clone();
        }
        for (locale, url) in &other.alternate_urls {
            self.alternate_urls.insert(locale.clone(), url.clone());
        }
        self.robots.extend(other.robots.iter().copied());
        self.json_ld.extend(other.json_ld.iter().cloned());
    }

    pub(crate) fn validate(&self) -> Result<(), WasmModelError> {
        if self
            .title
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
            || self
                .description
                .as_ref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "metadata title and description must be non-empty when present".to_string(),
            });
        }
        if let Some(canonical_url) = &self.canonical_url {
            let _ = validate_absolute_url("canonical_url", canonical_url.clone())?;
        }
        if self
            .alternate_urls
            .iter()
            .any(|(locale, url)| locale.trim().is_empty() || url.trim().is_empty())
        {
            return Err(WasmModelError::InvalidTypedReturn {
                reason: "metadata alternate URLs must be non-empty".to_string(),
            });
        }
        for (locale, url) in &self.alternate_urls {
            let _ = validate_token("alternate_url_locale", locale.clone())?;
            let _ = validate_absolute_url("alternate_url", url.clone())?;
        }
        for node in &self.json_ld {
            node.validate()?;
        }
        Ok(())
    }
}

fn validate_absolute_url(field: &'static str, value: String) -> Result<String, WasmModelError> {
    let trimmed = require_non_empty(field, value)?;
    if is_absolute_http_url(&trimmed) {
        Ok(trimmed)
    } else {
        Err(WasmModelError::InvalidTypedReturn {
            reason: format!("`{field}` must be an absolute URL"),
        })
    }
}

fn is_absolute_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}
