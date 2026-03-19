use std::collections::{BTreeMap, BTreeSet};

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;

use davenda_wasm::{CacheVisibility, ExecutionReceipt, TypedCacheHint, TypedMetadata};

use super::{FileDeliveryMode, LiveHtmlResponseGraph, render_cache_control};

#[derive(Debug, Clone)]
pub(crate) struct LiveResponseComposition {
    status: StatusCode,
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
    passthrough: Vec<LiveHeader>,
    cache_control: Option<HeaderValue>,
    surrogate_key: Option<HeaderValue>,
}

#[derive(Debug, Clone)]
struct LiveHeader {
    name: HeaderName,
    value: HeaderValue,
}

impl LiveHeader {
    fn new(name: HeaderName, value: HeaderValue) -> Self {
        Self { name, value }
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

    fn rendered_headers(&self) -> Vec<LiveHeader> {
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

#[derive(Debug, Clone)]
enum LiveResponseBody {
    Html(LiveHtmlResponseGraph),
    Json(BTreeMap<String, String>),
    Redirect {
        location: String,
    },
    File {
        logical_path: String,
        content_type: String,
        delivery_mode: FileDeliveryMode,
    },
}

impl LiveResponseComposition {
    pub(crate) fn html(status: StatusCode, body: LiveHtmlResponseGraph) -> Self {
        Self {
            status,
            cookies: Vec::new(),
            body: LiveResponseBody::Html(body),
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn json(status: StatusCode, body: BTreeMap<String, String>) -> Self {
        Self {
            status,
            cookies: Vec::new(),
            body: LiveResponseBody::Json(body),
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn redirect(status: StatusCode, location: impl Into<String>) -> Self {
        Self {
            status,
            cookies: Vec::new(),
            body: LiveResponseBody::Redirect {
                location: location.into(),
            },
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn file(
        status: StatusCode,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: FileDeliveryMode,
    ) -> Self {
        Self {
            status,
            cookies: Vec::new(),
            body: LiveResponseBody::File {
                logical_path: logical_path.into(),
                content_type: content_type.into(),
                delivery_mode,
            },
            annotations: LiveResponseAnnotations::default(),
        }
    }

    pub(crate) fn with_annotation(mut self, annotations: LiveResponseAnnotations) -> Self {
        self.annotations = annotations;
        self
    }

    pub(crate) fn with_cookie(mut self, value: impl Into<String>) -> Self {
        self.cookies.push(value.into());
        self
    }

    pub(crate) fn into_response(self) -> Response<Body> {
        let mut response = match self.body {
            LiveResponseBody::Html(body) => {
                body_response(self.status, body.render(), Some("text/html; charset=utf-8"))
            }
            LiveResponseBody::Json(payload) => {
                let body = render_json_object(payload);
                body_response(self.status, body, Some("application/json"))
            }
            LiveResponseBody::Redirect { location } => {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = self.status;
                response.headers_mut().insert(
                    HeaderName::from_static("location"),
                    HeaderValue::from_str(&location)
                        .expect("redirect location is a valid header value"),
                );
                response
            }
            LiveResponseBody::File {
                logical_path,
                content_type,
                delivery_mode,
            } => {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = self.status;
                response.headers_mut().insert(
                    HeaderName::from_static("content-type"),
                    HeaderValue::from_str(&content_type)
                        .expect("file content type is a valid header value"),
                );
                response.headers_mut().insert(
                    HeaderName::from_static("x-davenda-file-path"),
                    HeaderValue::from_str(&logical_path)
                        .expect("file logical path is a valid header value"),
                );
                response.headers_mut().insert(
                    HeaderName::from_static("x-davenda-file-delivery"),
                    HeaderValue::from_static(file_delivery_mode_name(delivery_mode)),
                );
                response
            }
        };

        for header in self.annotations.rendered_headers() {
            response.headers_mut().insert(header.name, header.value);
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

    fn rendered_headers(&self) -> Vec<LiveHeader> {
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

fn body_response(
    status: StatusCode,
    body: String,
    content_type: Option<&'static str>,
) -> Response<Body> {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    if let Some(content_type) = content_type {
        response.headers_mut().insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static(content_type),
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

fn receipt_headers(prefix: &str, receipt: &ExecutionReceipt) -> Vec<LiveHeader> {
    vec![
        header_value(
            &format!("x-davenda-wasm-{prefix}-handler"),
            receipt.handler_id.to_string(),
            "receipt handler id is a valid header value",
        ),
        header_value(
            &format!("x-davenda-wasm-{prefix}-point"),
            format!("{:?}", receipt.point),
            "receipt point is a valid header value",
        ),
        header_value(
            &format!("x-davenda-wasm-{prefix}-outcome"),
            format!("{:?}", receipt.outcome),
            "receipt outcome is a valid header value",
        ),
        header_value(
            &format!("x-davenda-wasm-{prefix}-runtime-ms"),
            receipt.runtime.as_millis().to_string(),
            "receipt runtime is a valid header value",
        ),
        header_value(
            &format!("x-davenda-wasm-{prefix}-host-calls"),
            receipt.host_calls.len().to_string(),
            "receipt host call count is a valid header value",
        ),
    ]
}

fn metadata_headers(metadata: &TypedMetadata) -> Vec<LiveHeader> {
    let mut headers = Vec::new();
    if let Some(title) = metadata.title.as_ref() {
        headers.push(header_value(
            "x-davenda-wasm-metadata-title",
            title.clone(),
            "metadata title is a valid header value",
        ));
    }
    if let Some(description) = metadata.description.as_ref() {
        headers.push(header_value(
            "x-davenda-wasm-metadata-description",
            description.clone(),
            "metadata description is a valid header value",
        ));
    }
    if let Some(canonical_url) = metadata.canonical_url.as_ref() {
        headers.push(header_value(
            "x-davenda-wasm-metadata-canonical",
            canonical_url.clone(),
            "metadata canonical URL is a valid header value",
        ));
    }
    if !metadata.alternate_urls.is_empty() {
        headers.push(header_value(
            "x-davenda-wasm-metadata-alternates",
            metadata
                .alternate_urls
                .iter()
                .map(|(locale, url)| format!("{locale}={url}"))
                .collect::<Vec<_>>()
                .join(","),
            "metadata alternates are a valid header value",
        ));
    }
    if !metadata.robots.is_empty() {
        headers.push(header_value(
            "x-davenda-wasm-metadata-robots",
            metadata
                .robots
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
            "metadata robots are a valid header value",
        ));
    }
    if !metadata.json_ld.is_empty() {
        headers.push(header_value(
            "x-davenda-wasm-metadata-json-ld-count",
            metadata.json_ld.len().to_string(),
            "metadata JSON-LD count is a valid header value",
        ));
    }
    headers
}

fn cache_hint_headers(cache_hint: &TypedCacheHint) -> Vec<LiveHeader> {
    let mut headers = vec![
        header_value(
            "x-davenda-wasm-cache-visibility",
            match cache_hint.visibility {
                CacheVisibility::Public => "public".to_string(),
                CacheVisibility::Private => "private".to_string(),
            },
            "cache visibility is a valid header value",
        ),
        header_value(
            "x-davenda-wasm-cache-control",
            render_cache_control(cache_hint),
            "cache control is a valid header value",
        ),
    ];
    if !cache_hint.tags.is_empty() {
        headers.push(header_value(
            "x-davenda-wasm-cache-tags",
            cache_hint
                .tags
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
            "cache tags are a valid header value",
        ));
    }
    headers
}

fn header_value(name: &str, value: String, reason: &'static str) -> LiveHeader {
    LiveHeader::new(
        HeaderName::try_from(name).expect("header name is static and valid"),
        HeaderValue::from_str(&value).expect(reason),
    )
}

fn file_delivery_mode_name(mode: FileDeliveryMode) -> &'static str {
    match mode {
        FileDeliveryMode::PublicCdn => "public_cdn",
        FileDeliveryMode::SignedUrl => "signed_url",
        FileDeliveryMode::AppProxy => "app_proxy",
        FileDeliveryMode::LocalOnly => "local_only",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
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

        let mut rendered = HeaderMap::new();
        for header in headers.rendered_headers() {
            rendered.insert(header.name, header.value);
        }

        assert_eq!(
            rendered.get("cache-control").unwrap(),
            "private,max-age=120,stale-while-revalidate=30,vary-by-locale,vary-by-session"
        );
        assert_eq!(
            rendered.get("surrogate-key").unwrap(),
            "locale-cache route-cache"
        );
        assert_eq!(rendered.get("X-Trace").unwrap(), "preserve");
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

    #[tokio::test]
    async fn html_response_composition_renders_structured_html_graphs() {
        let response = LiveResponseComposition::html(
            StatusCode::OK,
            crate::live::LiveHtmlResponseGraph::new("<html><body><main>base</main></body></html>")
                .with_request_surface(Some(&davenda_wasm::TypedExecutionOutput {
                    surface: davenda_wasm::ExtensionPointKind::Page,
                    status: 200,
                    body: davenda_wasm::TypedResponseBody::HtmlDocument(
                        "<html><body><section>request</section></body></html>".to_string(),
                    ),
                    metadata: davenda_wasm::TypedMetadata::new(),
                    cache_hint: None,
                }))
                .with_render_hook(
                    "hook-1",
                    Some(&davenda_wasm::TypedExecutionOutput {
                        surface: davenda_wasm::ExtensionPointKind::RenderHook,
                        status: 200,
                        body: davenda_wasm::TypedResponseBody::HtmlFragment(
                            "<aside>hook</aside>".to_string(),
                        ),
                        metadata: davenda_wasm::TypedMetadata::new(),
                        cache_hint: None,
                    }),
                ),
        );

        let response = response.into_response();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            body.as_ref(),
            b"<html><body><main>base</main><section>request</section><aside>hook</aside></body></html>"
        );
    }
}
