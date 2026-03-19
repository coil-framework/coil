use std::collections::BTreeMap;

use crate::error::WasmModelError;
use crate::ids::ExtensionPointKind;
use crate::validation::require_non_empty;

use super::cache::TypedCacheHint;
use super::metadata::TypedMetadata;

mod codec;
mod cursor;
mod mapping;

pub(super) const ABI_MAGIC: [u8; 4] = *b"DVRO";
pub(super) const ABI_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedResponseBody {
    HtmlDocument(String),
    HtmlFragment(String),
    JsonObject(BTreeMap<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypedResponseBodyKind {
    HtmlDocument,
    HtmlFragment,
    JsonObject,
}

impl std::fmt::Display for TypedResponseBodyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HtmlDocument => f.write_str("html_document"),
            Self::HtmlFragment => f.write_str("html_fragment"),
            Self::JsonObject => f.write_str("json_object"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypedExecutionOutput {
    pub surface: ExtensionPointKind,
    pub status: u16,
    pub body: TypedResponseBody,
    pub metadata: TypedMetadata,
    pub cache_hint: Option<TypedCacheHint>,
}

impl TypedExecutionOutput {
    pub const ABI_EXPORT: &'static str = "__davenda_typed_output";

    pub fn page(
        status: u16,
        body: impl Into<String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::Page,
            status,
            TypedResponseBody::HtmlDocument(require_non_empty("page_body", body.into())?),
            metadata,
            cache_hint,
        )
    }

    pub fn api(
        status: u16,
        payload: BTreeMap<String, String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::Api,
            status,
            TypedResponseBody::JsonObject(payload),
            metadata,
            cache_hint,
        )
    }

    pub fn admin_widget(
        status: u16,
        fragment: impl Into<String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::AdminWidget,
            status,
            TypedResponseBody::HtmlFragment(require_non_empty(
                "admin_widget_fragment",
                fragment.into(),
            )?),
            metadata,
            cache_hint,
        )
    }

    pub fn render_hook(
        status: u16,
        fragment: impl Into<String>,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        Self::new(
            ExtensionPointKind::RenderHook,
            status,
            TypedResponseBody::HtmlFragment(require_non_empty(
                "render_hook_fragment",
                fragment.into(),
            )?),
            metadata,
            cache_hint,
        )
    }

    pub(crate) fn new(
        surface: ExtensionPointKind,
        status: u16,
        body: TypedResponseBody,
        metadata: TypedMetadata,
        cache_hint: Option<TypedCacheHint>,
    ) -> Result<Self, WasmModelError> {
        let output = Self {
            surface,
            status,
            body,
            metadata,
            cache_hint,
        };
        output.validate_for_point(surface)?;
        Ok(output)
    }

    pub fn decode_page(bytes: &[u8]) -> Result<Self, WasmModelError> {
        Self::decode_for_point(bytes, ExtensionPointKind::Page)
    }

    pub fn decode_api(bytes: &[u8]) -> Result<Self, WasmModelError> {
        Self::decode_for_point(bytes, ExtensionPointKind::Api)
    }

    pub fn decode_admin_widget(bytes: &[u8]) -> Result<Self, WasmModelError> {
        Self::decode_for_point(bytes, ExtensionPointKind::AdminWidget)
    }

    pub fn decode_render_hook(bytes: &[u8]) -> Result<Self, WasmModelError> {
        Self::decode_for_point(bytes, ExtensionPointKind::RenderHook)
    }

    pub(crate) fn decode_for_point(
        bytes: &[u8],
        point: ExtensionPointKind,
    ) -> Result<Self, WasmModelError> {
        let output = codec::decode_output(bytes)?;
        output.validate_for_point(point)?;
        Ok(output)
    }

    pub(crate) fn validate_for_point(
        &self,
        point: ExtensionPointKind,
    ) -> Result<(), WasmModelError> {
        if self.surface != point {
            return Err(WasmModelError::TypedReturnPointMismatch {
                expected: point,
                actual: self.surface,
            });
        }

        if let Some(expected_body_kind) = mapping::expected_body_kind_for_point(point) {
            if self.body_kind() != expected_body_kind {
                return Err(WasmModelError::TypedReturnBodyMismatch {
                    point,
                    body: self.body_kind().to_string(),
                });
            }
        } else {
            return Err(WasmModelError::TypedReturnBodyMismatch {
                point,
                body: self.body_kind().to_string(),
            });
        }

        self.validate()
    }

    pub fn encode(&self) -> Result<Vec<u8>, WasmModelError> {
        self.validate_for_point(self.surface)?;
        codec::encode_output(self)
    }

    fn body_kind(&self) -> TypedResponseBodyKind {
        match &self.body {
            TypedResponseBody::HtmlDocument(_) => TypedResponseBodyKind::HtmlDocument,
            TypedResponseBody::HtmlFragment(_) => TypedResponseBodyKind::HtmlFragment,
            TypedResponseBody::JsonObject(_) => TypedResponseBodyKind::JsonObject,
        }
    }

    fn validate(&self) -> Result<(), WasmModelError> {
        mapping::validate_http_status(self.status)?;
        match &self.body {
            TypedResponseBody::HtmlDocument(html) | TypedResponseBody::HtmlFragment(html) => {
                let _ = require_non_empty("typed_response_body", html.clone())?;
            }
            TypedResponseBody::JsonObject(payload) => {
                for key in payload.keys() {
                    let _ = require_non_empty("json_object_key", key.clone())?;
                }
            }
        }
        self.metadata.validate()?;
        if let Some(cache) = &self.cache_hint {
            cache.validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
