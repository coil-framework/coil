use std::collections::{BTreeMap, BTreeSet};

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;

use davenda_wasm::{CacheVisibility, ExecutionReceipt, TypedCacheHint, TypedMetadata};

use super::{append_receipt_headers, insert_header, render_cache_control};

#[derive(Debug, Clone)]
pub(crate) struct LiveResponseComposition {
    status: StatusCode,
    headers: HeaderMap,
    cookies: Vec<String>,
    body: LiveResponseBody,
    annotations: LiveResponseAnnotations,
}

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
    passthrough: BTreeMap<String, String>,
    cache_control: Option<String>,
    surrogate_key: Option<String>,
}

impl LiveCacheHeaders {
    pub(crate) fn from_parts(
        headers: BTreeMap<String, String>,
        cache_hint: Option<&TypedCacheHint>,
    ) -> Self {
        let mut passthrough = headers;
        let cache_control = match cache_hint {
            Some(cache_hint) => Some(render_cache_control(cache_hint)),
            None => passthrough.remove("Cache-Control"),
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
                    passthrough.remove("Surrogate-Key")
                } else {
                    Some(rendered)
                }
            }
            None => passthrough.remove("Surrogate-Key"),
        };

        Self {
            passthrough,
            cache_control,
            surrogate_key,
        }
    }

    fn render_headers(&self) -> BTreeMap<String, String> {
        let mut rendered = self.passthrough.clone();
        if let Some(cache_control) = &self.cache_control {
            rendered.insert("Cache-Control".to_string(), cache_control.clone());
        }
        if let Some(surrogate_key) = &self.surrogate_key {
            rendered.insert("Surrogate-Key".to_string(), surrogate_key.clone());
        }
        rendered
    }
}

#[derive(Debug, Clone)]
enum LiveResponseBody {
    Html(String),
    Json(BTreeMap<String, String>),
    Empty,
}

impl LiveResponseComposition {
    pub(crate) fn html(status: StatusCode, body: String) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        Self {
            status,
            headers,
            cookies: Vec::new(),
            body: LiveResponseBody::Html(body),
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn json(status: StatusCode, body: BTreeMap<String, String>) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
        Self {
            status,
            headers,
            cookies: Vec::new(),
            body: LiveResponseBody::Json(body),
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn empty(status: StatusCode) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            cookies: Vec::new(),
            body: LiveResponseBody::Empty,
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn with_annotation(mut self, annotations: LiveResponseAnnotations) -> Self {
        self.annotations = annotations;
        self
    }

    pub(crate) fn with_header(mut self, name: impl AsRef<str>, value: impl Into<String>) -> Self {
        if let (Ok(header_name), Ok(header_value)) = (
            HeaderName::try_from(name.as_ref()),
            HeaderValue::from_str(&value.into()),
        ) {
            self.headers.insert(header_name, header_value);
        }
        self
    }

    pub(crate) fn with_cookie(mut self, value: impl Into<String>) -> Self {
        self.cookies.push(value.into());
        self
    }

    pub(crate) fn into_response(self) -> Response<Body> {
        let mut response = match self.body {
            LiveResponseBody::Html(body) => body_response(self.status, body, true),
            LiveResponseBody::Json(payload) => {
                let body = render_json_object(payload);
                body_response(self.status, body, false)
            }
            LiveResponseBody::Empty => {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = self.status;
                response
            }
        };

        render_annotations(response.headers_mut(), &self.annotations);
        for (name, value) in self.headers {
            if let Some(name) = name {
                response.headers_mut().insert(name, value);
            }
        }
        for cookie in self.cookies {
            if let Ok(value) = HeaderValue::from_str(&cookie) {
                response
                    .headers_mut()
                    .append(HeaderName::from_static("set-cookie"), value);
            }
        }

        response
    }
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
}

fn body_response(status: StatusCode, body: String, html: bool) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if html {
        response.headers_mut().insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
    }
    response
}

fn render_json_object(payload: BTreeMap<String, String>) -> String {
    let mut parts = Vec::new();
    for (key, value) in payload {
        parts.push(format!(
            "\"{}\":\"{}\"",
            escape_json(&key),
            escape_json(&value)
        ));
    }
    format!("{{{}}}", parts.join(","))
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn render_annotations(headers: &mut HeaderMap, annotations: &LiveResponseAnnotations) {
    if let Some(receipt) = &annotations.request_surface {
        append_receipt_headers(headers, "request", receipt);
    }

    if !annotations.render_hooks.is_empty() {
        headers.insert(
            HeaderName::from_static("x-davenda-wasm-render-hook-count"),
            HeaderValue::from_str(&annotations.render_hooks.len().to_string())
                .expect("render hook count is a valid header value"),
        );
        headers.insert(
            HeaderName::from_static("x-davenda-wasm-render-hook-handlers"),
            HeaderValue::from_str(
                &annotations
                    .render_hooks
                    .iter()
                    .map(|receipt| receipt.handler_id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .expect("render hook handler list is a valid header value"),
        );
        for receipt in &annotations.render_hooks {
            append_receipt_headers(headers, "render-hook", receipt);
        }
    }

    if !annotations.admin_widgets.is_empty() {
        headers.insert(
            HeaderName::from_static("x-davenda-wasm-admin-widget-count"),
            HeaderValue::from_str(&annotations.admin_widgets.len().to_string())
                .expect("admin widget count is a valid header value"),
        );
        headers.insert(
            HeaderName::from_static("x-davenda-wasm-admin-widget-handlers"),
            HeaderValue::from_str(
                &annotations
                    .admin_widgets
                    .iter()
                    .map(|receipt| receipt.handler_id.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .expect("admin widget handler list is a valid header value"),
        );
        for receipt in &annotations.admin_widgets {
            append_receipt_headers(headers, "admin-widget", receipt);
        }
    }

    if let Some(metadata) = &annotations.metadata {
        if let Some(title) = metadata.title.as_ref() {
            insert_header(headers, "x-davenda-wasm-metadata-title", title.clone());
        }
        if let Some(description) = metadata.description.as_ref() {
            insert_header(
                headers,
                "x-davenda-wasm-metadata-description",
                description.clone(),
            );
        }
        if let Some(canonical_url) = metadata.canonical_url.as_ref() {
            insert_header(
                headers,
                "x-davenda-wasm-metadata-canonical",
                canonical_url.clone(),
            );
        }
        if !metadata.alternate_urls.is_empty() {
            insert_header(
                headers,
                "x-davenda-wasm-metadata-alternates",
                metadata
                    .alternate_urls
                    .iter()
                    .map(|(locale, url)| format!("{locale}={url}"))
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        if !metadata.robots.is_empty() {
            insert_header(
                headers,
                "x-davenda-wasm-metadata-robots",
                metadata
                    .robots
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        if !metadata.json_ld.is_empty() {
            insert_header(
                headers,
                "x-davenda-wasm-metadata-json-ld-count",
                metadata.json_ld.len().to_string(),
            );
        }
    }

    if let Some(cache_hint) = &annotations.cache_hint {
        insert_header(
            headers,
            "x-davenda-wasm-cache-visibility",
            match cache_hint.visibility {
                CacheVisibility::Public => "public".to_string(),
                CacheVisibility::Private => "private".to_string(),
            },
        );
        insert_header(
            headers,
            "x-davenda-wasm-cache-control",
            render_cache_control(cache_hint),
        );
        if !cache_hint.tags.is_empty() {
            insert_header(
                headers,
                "x-davenda-wasm-cache-tags",
                cache_hint
                    .tags
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
    }

    if let Some(route) = annotations.route.as_ref() {
        insert_header(headers, "x-davenda-route", route.clone());
    }
    if let Some(locale) = annotations.locale.as_ref() {
        insert_header(headers, "x-davenda-locale", locale.clone());
    }

    for (name, value) in annotations.cache_headers.render_headers() {
        if let (Ok(header_name), Ok(header_value)) = (
            HeaderName::try_from(name.as_str()),
            HeaderValue::from_str(&value),
        ) {
            headers.insert(header_name, header_value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_wasm::{CacheVisibility, TypedCacheHint, TypedMetadata};

    #[test]
    fn cache_headers_use_structured_cache_hint_over_raw_string_values() {
        let cache_hint = TypedCacheHint::new(
            CacheVisibility::Private,
            120,
            Some(30),
            true,
            false,
            true,
            ["route-cache", "locale-cache"],
        )
        .unwrap();
        let headers = LiveCacheHeaders::from_parts(
            BTreeMap::from([
                (
                    "Cache-Control".to_string(),
                    "public, max-age=3600".to_string(),
                ),
                ("Surrogate-Key".to_string(), "stale legacy".to_string()),
                ("X-Trace".to_string(), "preserve".to_string()),
            ]),
            Some(&cache_hint),
        );

        assert_eq!(
            headers.render_headers().get("Cache-Control"),
            Some(
                &"private,max-age=120,stale-while-revalidate=30,vary-by-locale,vary-by-session"
                    .to_string()
            )
        );
        assert_eq!(
            headers.render_headers().get("Surrogate-Key"),
            Some(&"locale-cache route-cache".to_string())
        );
        assert_eq!(
            headers.render_headers().get("X-Trace"),
            Some(&"preserve".to_string())
        );
    }

    #[test]
    fn response_composition_materializes_typed_annotations_once() {
        let cache_hint = TypedCacheHint::new(
            CacheVisibility::Public,
            60,
            None,
            false,
            false,
            false,
            ["page-cache"],
        )
        .unwrap();
        let response = LiveResponseComposition::json(
            StatusCode::OK,
            BTreeMap::from([("ok".to_string(), "true".to_string())]),
        )
        .with_annotation(
            LiveResponseAnnotations::default()
                .metadata(Some(TypedMetadata::new().with_title("Demo").unwrap()))
                .cache_hint(Some(cache_hint.clone()))
                .cache_headers(LiveCacheHeaders::from_parts(
                    BTreeMap::from([("X-Trace".to_string(), "preserve".to_string())]),
                    Some(&cache_hint),
                ))
                .route("account.dashboard")
                .locale("en-GB"),
        );

        let response = response.into_response();
        assert_eq!(
            response
                .headers()
                .get("x-davenda-wasm-metadata-title")
                .unwrap(),
            "Demo"
        );
        assert_eq!(
            response
                .headers()
                .get("x-davenda-wasm-cache-control")
                .unwrap(),
            "public,max-age=60"
        );
        assert_eq!(
            response.headers().get("x-davenda-route").unwrap(),
            "account.dashboard"
        );
        assert_eq!(response.headers().get("x-davenda-locale").unwrap(), "en-GB");
        assert_eq!(response.headers().get("X-Trace").unwrap(), "preserve");
    }
}
