use std::collections::{BTreeMap, BTreeSet};

use axum::http::{HeaderName, HeaderValue};
use davenda_wasm::{ExecutionReceipt, TypedCacheHint, TypedMetadata};

use super::headers::{
    LiveHeader, cache_hint_headers, header_value, metadata_headers, receipt_headers,
    render_cache_control,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct LiveResponseAnnotations {
    request_surface: Option<ExecutionReceipt>,
    render_hooks: Vec<ExecutionReceipt>,
    admin_widgets: Vec<ExecutionReceipt>,
    metadata: Option<TypedMetadata>,
    cache_hint: Option<TypedCacheHint>,
    cache_headers: LiveCacheHeaders,
    route: Option<String>,
    locale: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LiveCacheHeaders {
    passthrough: Vec<LiveHeader>,
    cache_control: Option<HeaderValue>,
    surrogate_key: Option<HeaderValue>,
}

impl LiveResponseAnnotations {
    pub(crate) fn request_surface(mut self, receipt: Option<ExecutionReceipt>) -> Self {
        self.request_surface = receipt;
        self
    }

    pub(crate) fn render_hooks(mut self, receipts: Vec<ExecutionReceipt>) -> Self {
        self.render_hooks = receipts;
        self
    }

    pub(crate) fn admin_widgets(mut self, receipts: Vec<ExecutionReceipt>) -> Self {
        self.admin_widgets = receipts;
        self
    }

    pub(crate) fn metadata(mut self, metadata: Option<TypedMetadata>) -> Self {
        self.metadata = metadata;
        self
    }

    pub(crate) fn cache_hint(mut self, cache_hint: Option<TypedCacheHint>) -> Self {
        self.cache_hint = cache_hint;
        self
    }

    pub(crate) fn cache_headers(mut self, headers: LiveCacheHeaders) -> Self {
        self.cache_headers = headers;
        self
    }

    pub(crate) fn route(mut self, route: impl Into<String>) -> Self {
        self.route = Some(route.into());
        self
    }

    pub(crate) fn locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    pub(super) fn rendered_headers(&self) -> Vec<LiveHeader> {
        let mut headers = Vec::new();

        if let Some(receipt) = &self.request_surface {
            headers.extend(receipt_headers("request", receipt));
        }

        if !self.render_hooks.is_empty() {
            headers.push(LiveHeader::new(
                HeaderName::from_static("x-davenda-wasm-render-hook-count"),
                HeaderValue::from_str(&self.render_hooks.len().to_string())
                    .expect("render hook count is a valid header value"),
            ));
            headers.push(LiveHeader::new(
                HeaderName::from_static("x-davenda-wasm-render-hook-handlers"),
                HeaderValue::from_str(
                    &self
                        .render_hooks
                        .iter()
                        .map(|receipt| receipt.handler_id.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .expect("render hook handler list is a valid header value"),
            ));
            for receipt in &self.render_hooks {
                headers.extend(receipt_headers("render-hook", receipt));
            }
        }

        if !self.admin_widgets.is_empty() {
            headers.push(LiveHeader::new(
                HeaderName::from_static("x-davenda-wasm-admin-widget-count"),
                HeaderValue::from_str(&self.admin_widgets.len().to_string())
                    .expect("admin widget count is a valid header value"),
            ));
            headers.push(LiveHeader::new(
                HeaderName::from_static("x-davenda-wasm-admin-widget-handlers"),
                HeaderValue::from_str(
                    &self
                        .admin_widgets
                        .iter()
                        .map(|receipt| receipt.handler_id.to_string())
                        .collect::<Vec<_>>()
                        .join(","),
                )
                .expect("admin widget handler list is a valid header value"),
            ));
            for receipt in &self.admin_widgets {
                headers.extend(receipt_headers("admin-widget", receipt));
            }
        }

        if let Some(metadata) = &self.metadata {
            headers.extend(metadata_headers(metadata));
        }

        if let Some(cache_hint) = &self.cache_hint {
            headers.extend(cache_hint_headers(cache_hint));
        }

        if let Some(route) = self.route.as_ref() {
            headers.push(header_value(
                "x-davenda-route",
                route.clone(),
                "route is a valid header value",
            ));
        }
        if let Some(locale) = self.locale.as_ref() {
            headers.push(header_value(
                "x-davenda-locale",
                locale.clone(),
                "locale is a valid header value",
            ));
        }

        headers.extend(self.cache_headers.rendered_headers());
        headers
    }
}

impl LiveCacheHeaders {
    pub(crate) fn from_parts(
        headers: BTreeMap<String, String>,
        cache_hint: Option<&TypedCacheHint>,
    ) -> Self {
        let mut passthrough = Vec::new();
        let cache_control = match cache_hint {
            Some(cache_hint) => HeaderValue::from_str(&render_cache_control(cache_hint)).ok(),
            None => None,
        };
        let surrogate_key = match cache_hint {
            Some(cache_hint) => {
                let rendered = cache_hint
                    .tags
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(" ");
                if rendered.is_empty() {
                    None
                } else {
                    HeaderValue::from_str(&rendered).ok()
                }
            }
            None => None,
        };

        for (name, value) in headers {
            if cache_hint.is_some() && matches!(name.as_str(), "Cache-Control" | "Surrogate-Key") {
                continue;
            }

            if let (Ok(header_name), Ok(header_value)) = (
                HeaderName::try_from(name.as_str()),
                HeaderValue::from_str(&value),
            ) {
                passthrough.push(LiveHeader::new(header_name, header_value));
            }
        }

        Self {
            passthrough,
            cache_control,
            surrogate_key,
        }
    }

    pub(super) fn rendered_headers(&self) -> Vec<LiveHeader> {
        let mut headers = self.passthrough.clone();
        if let Some(cache_control) = &self.cache_control {
            headers.push(LiveHeader::new(
                HeaderName::from_static("cache-control"),
                cache_control.clone(),
            ));
        }
        if let Some(surrogate_key) = &self.surrogate_key {
            headers.push(LiveHeader::new(
                HeaderName::from_static("surrogate-key"),
                surrogate_key.clone(),
            ));
        }
        headers
    }
}
