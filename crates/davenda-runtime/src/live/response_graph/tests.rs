use std::collections::BTreeMap;

use axum::body::to_bytes;
use axum::http::{HeaderMap, StatusCode};
use davenda_wasm::{CacheVisibility, TypedCacheHint, TypedMetadata};

use super::{LiveCacheHeaders, LiveResponseAnnotations, LiveResponseComposition};

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
    let cache_hint =
        TypedCacheHint::new(CacheVisibility::Public, 60, None, false, false, false, ["page-cache"])
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
