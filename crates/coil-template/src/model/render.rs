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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderModelMergePolicy {
    FailOnConflict,
    ReplaceExisting,
    AppendLists,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RenderModel {
    values: BTreeMap<String, RenderValue>,
    asset_paths: BTreeMap<String, String>,
    translations: BTreeMap<String, String>,
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

    pub fn with_translation(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, TemplateModelError> {
        let key = validate_token("translation_key", key.into())?;
        let value = require_non_empty("translation_value", value.into())?;
        self.translations.insert(key, value);
        Ok(self)
    }

    pub fn mount_object(
        mut self,
        path: impl AsRef<str>,
        value: RenderModel,
    ) -> Result<Self, TemplateModelError> {
        let segments = validate_render_model_path(path.as_ref())?;
        merge_named_string_maps(
            &mut self.asset_paths,
            &value.asset_paths,
            "asset_path",
            RenderModelMergePolicy::FailOnConflict,
        )?;
        merge_named_string_maps(
            &mut self.translations,
            &value.translations,
            "translation",
            RenderModelMergePolicy::FailOnConflict,
        )?;
        self.mount_object_segments(&segments, value)?;
        Ok(self)
    }

    pub fn merge_object(
        mut self,
        path: impl AsRef<str>,
        value: RenderModel,
        policy: RenderModelMergePolicy,
    ) -> Result<Self, TemplateModelError> {
        let segments = validate_render_model_path(path.as_ref())?;
        merge_named_string_maps(&mut self.asset_paths, &value.asset_paths, "asset_path", policy)?;
        merge_named_string_maps(
            &mut self.translations,
            &value.translations,
            "translation",
            policy,
        )?;
        self.merge_object_segments(&segments, value, policy)?;
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

    pub(crate) fn get_translation(&self, key: &str) -> Option<&str> {
        self.translations.get(key).map(String::as_str)
    }

    pub(crate) fn merged_with(&self, overlay: &RenderModel) -> RenderModel {
        let mut values = self.values.clone();
        values.extend(overlay.values.clone());
        let mut asset_paths = self.asset_paths.clone();
        asset_paths.extend(overlay.asset_paths.clone());
        let mut translations = self.translations.clone();
        translations.extend(overlay.translations.clone());
        RenderModel {
            values,
            asset_paths,
            translations,
        }
    }

    fn mount_object_segments(
        &mut self,
        segments: &[String],
        value: RenderModel,
    ) -> Result<(), TemplateModelError> {
        let head = &segments[0];
        if segments.len() == 1 {
            if self.values.contains_key(head) {
                return Err(TemplateModelError::RenderModelConflict {
                    path: head.clone(),
                    message: "mount target already exists".to_string(),
                });
            }
            self.values.insert(head.clone(), RenderValue::object(value));
            return Ok(());
        }

        let existing = self.values.remove(head);
        let mut child = match existing {
            Some(RenderValue::Object(child)) => child,
            Some(_) => {
                return Err(TemplateModelError::RenderModelConflict {
                    path: head.clone(),
                    message: "mount target traverses a non-object value".to_string(),
                });
            }
            None => RenderModel::new(),
        };
        child.mount_object_segments(&segments[1..], value)?;
        self.values.insert(head.clone(), RenderValue::object(child));
        Ok(())
    }

    fn merge_object_segments(
        &mut self,
        segments: &[String],
        value: RenderModel,
        policy: RenderModelMergePolicy,
    ) -> Result<(), TemplateModelError> {
        let head = &segments[0];
        if segments.len() == 1 {
            match self.values.remove(head) {
                Some(RenderValue::Object(mut existing)) => {
                    merge_render_model_objects(&mut existing, value, policy, head.as_str())?;
                    self.values
                        .insert(head.clone(), RenderValue::object(existing));
                }
                Some(_) => {
                    return Err(TemplateModelError::RenderModelConflict {
                        path: head.clone(),
                        message: "merge target must be an object".to_string(),
                    });
                }
                None => {
                    self.values.insert(head.clone(), RenderValue::object(value));
                }
            }
            return Ok(());
        }

        let existing = self.values.remove(head);
        let mut child = match existing {
            Some(RenderValue::Object(child)) => child,
            Some(_) => {
                return Err(TemplateModelError::RenderModelConflict {
                    path: head.clone(),
                    message: "merge target traverses a non-object value".to_string(),
                });
            }
            None => RenderModel::new(),
        };
        child.merge_object_segments(&segments[1..], value, policy)?;
        self.values.insert(head.clone(), RenderValue::object(child));
        Ok(())
    }
}

fn validate_render_model_path(path: &str) -> Result<Vec<String>, TemplateModelError> {
    let path = require_non_empty("render_model_path", path.trim().to_string())?;
    path.split('.')
        .map(|segment| validate_token("render_key", segment.to_string()))
        .collect()
}

fn merge_named_string_maps(
    target: &mut BTreeMap<String, String>,
    overlay: &BTreeMap<String, String>,
    label: &str,
    policy: RenderModelMergePolicy,
) -> Result<(), TemplateModelError> {
    for (key, value) in overlay {
        match target.get(key) {
            None => {
                target.insert(key.clone(), value.clone());
            }
            Some(existing) if existing == value => {}
            Some(_) => match policy {
                RenderModelMergePolicy::FailOnConflict | RenderModelMergePolicy::AppendLists => {
                    return Err(TemplateModelError::RenderModelConflict {
                        path: format!("{label}:{key}"),
                        message: "existing value differs from contribution".to_string(),
                    });
                }
                RenderModelMergePolicy::ReplaceExisting => {
                    target.insert(key.clone(), value.clone());
                }
            },
        }
    }
    Ok(())
}

fn merge_render_model_objects(
    target: &mut RenderModel,
    overlay: RenderModel,
    policy: RenderModelMergePolicy,
    path_prefix: &str,
) -> Result<(), TemplateModelError> {
    merge_named_string_maps(
        &mut target.asset_paths,
        &overlay.asset_paths,
        "asset_path",
        policy,
    )?;
    merge_named_string_maps(
        &mut target.translations,
        &overlay.translations,
        "translation",
        policy,
    )?;

    for (key, value) in overlay.values {
        let path = if path_prefix.is_empty() {
            key.clone()
        } else {
            format!("{path_prefix}.{key}")
        };
        match target.values.remove(&key) {
            None => {
                target.values.insert(key, value);
            }
            Some(existing) => {
                let merged = merge_render_value(existing, value, policy, path.as_str())?;
                target.values.insert(key, merged);
            }
        }
    }

    Ok(())
}

fn merge_render_value(
    existing: RenderValue,
    overlay: RenderValue,
    policy: RenderModelMergePolicy,
    path: &str,
) -> Result<RenderValue, TemplateModelError> {
    match (existing, overlay) {
        (RenderValue::Object(mut existing), RenderValue::Object(overlay)) => {
            merge_render_model_objects(&mut existing, overlay, policy, path)?;
            Ok(RenderValue::object(existing))
        }
        (RenderValue::List(mut existing), RenderValue::List(overlay)) => match policy {
            RenderModelMergePolicy::AppendLists => {
                existing.extend(overlay);
                Ok(RenderValue::list(existing))
            }
            RenderModelMergePolicy::ReplaceExisting => Ok(RenderValue::list(overlay)),
            RenderModelMergePolicy::FailOnConflict => Err(
                TemplateModelError::RenderModelConflict {
                    path: path.to_string(),
                    message: "list values conflict; use append_lists or replace_existing"
                        .to_string(),
                },
            ),
        },
        (existing, overlay) if existing == overlay => Ok(existing),
        (_existing, overlay) => match policy {
            RenderModelMergePolicy::ReplaceExisting => Ok(overlay),
            RenderModelMergePolicy::FailOnConflict | RenderModelMergePolicy::AppendLists => {
                Err(TemplateModelError::RenderModelConflict {
                    path: path.to_string(),
                    message: "existing value differs from contribution".to_string(),
                })
            }
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutput {
    pub html: String,
}
