use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;
use davenda_wasm::{CacheVisibility, ExecutionReceipt, TypedCacheHint, TypedMetadata};

use crate::FileDeliveryMode;

#[derive(Debug, Clone)]
pub(super) struct LiveHeader {
    pub(super) name: HeaderName,
    pub(super) value: HeaderValue,
}

impl LiveHeader {
    pub(super) fn new(name: HeaderName, value: HeaderValue) -> Self {
        Self { name, value }
    }
}

pub(super) fn body_response(
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

pub(super) fn render_json_object(payload: BTreeMap<String, String>) -> String {
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

pub(super) fn receipt_headers(prefix: &str, receipt: &ExecutionReceipt) -> Vec<LiveHeader> {
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

pub(super) fn metadata_headers(metadata: &TypedMetadata) -> Vec<LiveHeader> {
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

pub(super) fn cache_hint_headers(cache_hint: &TypedCacheHint) -> Vec<LiveHeader> {
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
            cache_hint.tags.iter().cloned().collect::<Vec<_>>().join(","),
            "cache tags are a valid header value",
        ));
    }
    headers
}

pub(super) fn header_value(name: &str, value: String, reason: &'static str) -> LiveHeader {
    LiveHeader::new(
        HeaderName::try_from(name).expect("header name is static and valid"),
        HeaderValue::from_str(&value).expect(reason),
    )
}

pub(super) fn render_cache_control(cache_hint: &TypedCacheHint) -> String {
    let mut directives = Vec::new();
    directives.push(match cache_hint.visibility {
        CacheVisibility::Public => "public".to_string(),
        CacheVisibility::Private => "private".to_string(),
    });
    directives.push(format!("max-age={}", cache_hint.max_age_seconds));
    if let Some(value) = cache_hint.stale_while_revalidate_seconds {
        directives.push(format!("stale-while-revalidate={value}"));
    }
    if cache_hint.vary_by_locale {
        directives.push("vary-by-locale".to_string());
    }
    if cache_hint.vary_by_user {
        directives.push("vary-by-user".to_string());
    }
    if cache_hint.vary_by_session {
        directives.push("vary-by-session".to_string());
    }
    directives.join(",")
}

pub(super) fn file_delivery_mode_name(mode: FileDeliveryMode) -> &'static str {
    match mode {
        FileDeliveryMode::PublicCdn => "public_cdn",
        FileDeliveryMode::SignedUrl => "signed_url",
        FileDeliveryMode::AppProxy => "app_proxy",
        FileDeliveryMode::LocalOnly => "local_only",
    }
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
