use super::*;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};

#[derive(Debug, Clone, Default)]
pub(crate) struct LiveExecutionReceipts {
    request_surface: Option<ExecutionReceipt>,
    render_hooks: Vec<ExecutionReceipt>,
    admin_widgets: Vec<ExecutionReceipt>,
}

impl LiveExecutionReceipts {
    pub(crate) fn collect(
        plan: &RuntimePlan,
        execution: &RequestExecution,
    ) -> Result<Self, RuntimeServerError> {
        let wasm = plan.wasm_host();
        let request_surface = wasm.execute_request_surface(execution)?;

        let render_hooks = if matches!(
            execution.response,
            HandlerResponse::Page(_) | HandlerResponse::Fragment(_)
        ) {
            let mut receipts = Vec::new();
            for slot in render_hook_slots_for_execution(plan, execution) {
                receipts.extend(wasm.execute_render_hook_slot(slot.as_str(), execution)?);
            }
            receipts
        } else {
            Vec::new()
        };

        let admin_widgets = if execution.route_area == RouteArea::Admin {
            let mut receipts = Vec::new();
            for slot in admin_widget_slots_for_execution(plan, execution) {
                receipts.extend(wasm.execute_admin_widget_slot(slot.as_str(), execution)?);
            }
            receipts
        } else {
            Vec::new()
        };

        Ok(Self {
            request_surface,
            render_hooks,
            admin_widgets,
        })
    }

    pub(crate) fn request_surface_output(&self) -> Option<&TypedExecutionOutput> {
        self.request_surface
            .as_ref()
            .and_then(|receipt| receipt.typed_output.as_ref())
    }

    pub(crate) fn response_status(&self, base: StatusCode) -> StatusCode {
        self.request_surface_output()
            .and_then(|output| StatusCode::from_u16(output.status).ok())
            .unwrap_or(base)
    }

    pub(crate) fn merged_metadata(&self) -> Option<TypedMetadata> {
        let mut merged: Option<TypedMetadata> = None;
        for output in self.typed_outputs() {
            if let Some(metadata) = &mut merged {
                metadata.merge_from(&output.metadata);
            } else {
                merged = Some(output.metadata.clone());
            }
        }
        merged
    }

    pub(crate) fn merged_cache_hint(&self) -> Option<TypedCacheHint> {
        let mut merged: Option<TypedCacheHint> = None;
        for output in self.typed_outputs() {
            if let Some(cache_hint) = &output.cache_hint {
                if let Some(existing) = &mut merged {
                    existing.merge_from(cache_hint);
                } else {
                    merged = Some(cache_hint.clone());
                }
            }
        }
        merged
    }

    pub(crate) fn compose_page_html(&self, html: String) -> String {
        let mut html = html;
        let mut body_fragments = Vec::new();

        if let Some(output) = self.request_surface_output()
            && let Some(fragment) = typed_output_fragment(output)
        {
            body_fragments.push(fragment);
        }

        for output in self.render_hook_outputs() {
            if let Some(fragment) = typed_output_fragment(output) {
                body_fragments.push(fragment);
            }
        }

        for output in self.admin_widget_outputs() {
            if let Some(fragment) = typed_output_fragment(output) {
                body_fragments.push(fragment);
            }
        }

        if !body_fragments.is_empty() {
            html = inject_body_markup(html, &body_fragments.join(""));
        }

        html
    }

    pub(crate) fn compose_fragment_html(&self, html: String) -> String {
        let mut html = html;
        let mut body_fragments = Vec::new();

        if let Some(output) = self.request_surface_output()
            && let Some(fragment) = typed_output_fragment(output)
        {
            body_fragments.push(fragment);
        }

        for output in self.render_hook_outputs() {
            if let Some(fragment) = typed_output_fragment(output) {
                body_fragments.push(fragment);
            }
        }

        for output in self.admin_widget_outputs() {
            if let Some(fragment) = typed_output_fragment(output) {
                body_fragments.push(fragment);
            }
        }

        if !body_fragments.is_empty() {
            html = inject_body_markup(html, &body_fragments.join(""));
        }

        html
    }

    pub(crate) fn compose_json_payload(
        &self,
        mut payload: BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        if let Some(output) = self.request_surface_output()
            && let TypedResponseBody::JsonObject(typed_payload) = &output.body
        {
            payload.extend(typed_payload.clone());
        }

        payload
    }

    pub(crate) fn compose_cache_headers(
        &self,
        mut headers: BTreeMap<String, String>,
    ) -> BTreeMap<String, String> {
        if let Some(cache_hint) = self.merged_cache_hint() {
            let merged_cache_control = headers
                .get("Cache-Control")
                .map(|value| merge_cache_control_value(value, &cache_hint))
                .unwrap_or_else(|| render_cache_control(&cache_hint));
            headers.insert("Cache-Control".to_string(), merged_cache_control);

            let merged_surrogate_tags = headers
                .get("Surrogate-Key")
                .map(|value| merge_surrogate_tags(value, &cache_hint))
                .unwrap_or_else(|| {
                    cache_hint
                        .tags
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" ")
                });
            if !merged_surrogate_tags.is_empty() {
                headers.insert("Surrogate-Key".to_string(), merged_surrogate_tags);
            }
        }

        headers
    }

    pub(crate) fn decorate_response_headers(&self, headers: &mut HeaderMap) {
        if let Some(receipt) = &self.request_surface {
            append_receipt_headers(headers, "request", receipt);
        }

        if !self.render_hooks.is_empty() {
            headers.insert(
                HeaderName::from_static("x-davenda-wasm-render-hook-count"),
                HeaderValue::from_str(&self.render_hooks.len().to_string())
                    .expect("render hook count is a valid header value"),
            );
            headers.insert(
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
            );
            for receipt in &self.render_hooks {
                append_receipt_headers(headers, "render-hook", receipt);
            }
        }

        if !self.admin_widgets.is_empty() {
            headers.insert(
                HeaderName::from_static("x-davenda-wasm-admin-widget-count"),
                HeaderValue::from_str(&self.admin_widgets.len().to_string())
                    .expect("admin widget count is a valid header value"),
            );
            headers.insert(
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
            );
            for receipt in &self.admin_widgets {
                append_receipt_headers(headers, "admin-widget", receipt);
            }
        }

        if let Some(metadata) = self.merged_metadata() {
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

        if let Some(cache_hint) = self.merged_cache_hint() {
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
                render_cache_control(&cache_hint),
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
    }

    fn typed_outputs(&self) -> Vec<&TypedExecutionOutput> {
        let mut outputs = Vec::new();
        if let Some(receipt) = &self.request_surface
            && let Some(output) = receipt.typed_output.as_ref()
        {
            outputs.push(output);
        }
        outputs.extend(
            self.render_hooks
                .iter()
                .filter_map(|receipt| receipt.typed_output.as_ref()),
        );
        outputs.extend(
            self.admin_widgets
                .iter()
                .filter_map(|receipt| receipt.typed_output.as_ref()),
        );
        outputs
    }

    fn render_hook_outputs(&self) -> Vec<&TypedExecutionOutput> {
        self.render_hooks
            .iter()
            .filter_map(|receipt| receipt.typed_output.as_ref())
            .collect()
    }

    fn admin_widget_outputs(&self) -> Vec<&TypedExecutionOutput> {
        self.admin_widgets
            .iter()
            .filter_map(|receipt| receipt.typed_output.as_ref())
            .collect()
    }
}

fn append_receipt_headers(headers: &mut HeaderMap, prefix: &str, receipt: &ExecutionReceipt) {
    insert_header(
        headers,
        &format!("x-davenda-wasm-{prefix}-handler"),
        receipt.handler_id.to_string(),
    );
    insert_header(
        headers,
        &format!("x-davenda-wasm-{prefix}-point"),
        format!("{:?}", receipt.point),
    );
    insert_header(
        headers,
        &format!("x-davenda-wasm-{prefix}-outcome"),
        format!("{:?}", receipt.outcome),
    );
    insert_header(
        headers,
        &format!("x-davenda-wasm-{prefix}-runtime-ms"),
        receipt.runtime.as_millis().to_string(),
    );
    insert_header(
        headers,
        &format!("x-davenda-wasm-{prefix}-host-calls"),
        receipt.host_calls.len().to_string(),
    );
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: String) {
    if let Ok(header_name) = HeaderName::try_from(name) {
        if let Ok(header_value) = HeaderValue::from_str(&value) {
            headers.insert(header_name, header_value);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CacheControlPolicy {
    visibility: CacheVisibility,
    max_age_seconds: u64,
    stale_while_revalidate_seconds: Option<u64>,
}

impl CacheControlPolicy {
    fn parse(value: &str) -> Option<Self> {
        let mut visibility = None;
        let mut max_age_seconds = None;
        let mut stale_while_revalidate_seconds = None;

        for directive in value.split(',') {
            let directive = directive.trim();
            if directive.is_empty() {
                continue;
            }
            if directive == "no-store" {
                return None;
            }
            if directive == "public" {
                visibility = Some(CacheVisibility::Public);
                continue;
            }
            if directive == "private" {
                visibility = Some(CacheVisibility::Private);
                continue;
            }
            if let Some(value) = directive.strip_prefix("max-age=") {
                max_age_seconds = value.parse::<u64>().ok();
                continue;
            }
            if let Some(value) = directive.strip_prefix("stale-while-revalidate=") {
                stale_while_revalidate_seconds = value.parse::<u64>().ok();
            }
        }

        Some(Self {
            visibility: visibility?,
            max_age_seconds: max_age_seconds?,
            stale_while_revalidate_seconds,
        })
    }

    fn merge_from_hint(&mut self, cache_hint: &TypedCacheHint) {
        self.visibility = match (self.visibility, cache_hint.visibility) {
            (CacheVisibility::Private, _) | (_, CacheVisibility::Private) => {
                CacheVisibility::Private
            }
            _ => CacheVisibility::Public,
        };
        self.max_age_seconds = self.max_age_seconds.min(cache_hint.max_age_seconds);
        self.stale_while_revalidate_seconds = match (
            self.stale_while_revalidate_seconds,
            cache_hint.stale_while_revalidate_seconds,
        ) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (Some(left), None) => Some(left),
            (None, Some(right)) => Some(right),
            (None, None) => None,
        };
    }

    fn render(self) -> String {
        let mut directives = Vec::new();
        directives.push(match self.visibility {
            CacheVisibility::Public => "public".to_string(),
            CacheVisibility::Private => "private".to_string(),
        });
        directives.push(format!("max-age={}", self.max_age_seconds));
        if let Some(value) = self.stale_while_revalidate_seconds {
            directives.push(format!("stale-while-revalidate={value}"));
        }
        directives.join(", ")
    }
}

fn merge_cache_control_value(existing: &str, cache_hint: &TypedCacheHint) -> String {
    if existing.trim() == "no-store" {
        return "no-store".to_string();
    }

    let Some(mut policy) = CacheControlPolicy::parse(existing) else {
        return existing.to_string();
    };
    policy.merge_from_hint(cache_hint);
    policy.render()
}

fn merge_surrogate_tags(existing: &str, cache_hint: &TypedCacheHint) -> String {
    let mut tags = BTreeSet::new();
    tags.extend(
        existing
            .split_whitespace()
            .filter(|tag| !tag.is_empty())
            .map(str::to_string),
    );
    tags.extend(cache_hint.tags.iter().cloned());
    tags.into_iter().collect::<Vec<_>>().join(" ")
}

fn render_cache_control(cache_hint: &TypedCacheHint) -> String {
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

fn inject_body_markup(document_html: String, body_markup: &str) -> String {
    if body_markup.is_empty() {
        return document_html;
    }

    if let Some(index) = document_html.find("</body>") {
        let mut html = document_html;
        html.insert_str(index, body_markup);
        return html;
    }

    format!("{document_html}{body_markup}")
}

fn typed_output_fragment(output: &TypedExecutionOutput) -> Option<String> {
    match &output.body {
        TypedResponseBody::HtmlDocument(html) => Some(document_body_fragment(html)),
        TypedResponseBody::HtmlFragment(html) => Some(html.clone()),
        TypedResponseBody::JsonObject(_) => None,
    }
}

fn document_body_fragment(document_html: &str) -> String {
    let Some(body_start) = document_html.find("<body") else {
        return document_html.to_string();
    };
    let Some(body_open_end) = document_html[body_start..].find('>') else {
        return document_html.to_string();
    };
    let content_start = body_start + body_open_end + 1;
    let Some(body_close) = document_html[content_start..].find("</body>") else {
        return document_html.to_string();
    };
    document_html[content_start..content_start + body_close].to_string()
}

fn render_hook_slots_for_execution(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> Vec<String> {
    let module = plan
        .http
        .routes
        .iter()
        .find(|route| route.name == execution.route.route_name)
        .and_then(|route| route.module.as_deref());

    plan.registered_extension_slots
        .iter()
        .filter(|slot| {
            slot.kind == ExtensionPointKind::RenderHook && Some(slot.module.as_str()) == module
        })
        .map(|slot| slot.surface.clone())
        .collect()
}

fn admin_widget_slots_for_execution(
    plan: &RuntimePlan,
    execution: &RequestExecution,
) -> Vec<String> {
    let module = plan
        .http
        .routes
        .iter()
        .find(|route| route.name == execution.route.route_name)
        .and_then(|route| route.module.as_deref());

    plan.registered_extension_slots
        .iter()
        .filter(|slot| {
            slot.kind == ExtensionPointKind::AdminWidget && Some(slot.module.as_str()) == module
        })
        .map(|slot| slot.surface.clone())
        .collect()
}
