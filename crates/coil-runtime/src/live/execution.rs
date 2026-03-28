use super::graph::LiveJsonResponseGraph;
use super::*;
use axum::http::StatusCode;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct LiveExecutionReceipts {
    request_surface: Option<ExecutionReceipt>,
    render_hooks: Vec<ExecutionReceipt>,
    admin_widgets: Vec<ExecutionReceipt>,
}

impl LiveExecutionReceipts {
    pub(crate) fn collect(
        plan: &RuntimePlan,
        wasm_host: &WasmHost,
        execution: &RequestExecution,
    ) -> Result<Self, RuntimeServerError> {
        let request_surface = wasm_host.execute_request_surface(execution)?;

        let render_hooks = if matches!(
            execution.response,
            HandlerResponse::Page(_) | HandlerResponse::Fragment(_)
        ) {
            let mut receipts = Vec::new();
            for slot in render_hook_slots_for_execution(plan, execution) {
                receipts.extend(wasm_host.execute_render_hook_slot(slot.as_str(), execution)?);
            }
            receipts
        } else {
            Vec::new()
        };

        let admin_widgets = if execution.route_area == RouteArea::Admin {
            let mut receipts = Vec::new();
            for slot in admin_widget_slots_for_execution(plan, execution) {
                receipts.extend(wasm_host.execute_admin_widget_slot(slot.as_str(), execution)?);
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

    pub(crate) fn as_annotations(&self) -> LiveResponseAnnotations {
        LiveResponseAnnotations::default()
            .request_surface(self.request_surface.clone())
            .render_hooks(self.render_hooks.clone())
            .admin_widgets(self.admin_widgets.clone())
            .metadata(self.merged_metadata())
            .cache_hint(self.merged_cache_hint())
    }

    pub(crate) fn compose_response(
        &self,
        plan: &RuntimePlan,
        execution: &RequestExecution,
    ) -> Result<LiveResponseComposition, RuntimeServerError> {
        let metadata = self.merged_metadata();
        let cache_hint = self.merged_cache_hint();
        let annotations = self
            .as_annotations()
            .cache_headers(LiveCacheHeaders::from_parts(
                execution.cache_plan.headers.clone(),
                cache_hint.as_ref(),
            ))
            .route(execution.route.route_name.clone())
            .locale(execution.locale.clone());

        let mut response = match &execution.response {
            HandlerResponse::Page(page) => {
                let html = plan.render_page_response(execution, page, metadata.as_ref())?;
                let status = self
                    .response_status(StatusCode::from_u16(page.status).unwrap_or(StatusCode::OK));
                LiveResponseComposition::html(status, self.compose_html_graph(html))
            }
            HandlerResponse::Fragment(fragment) => {
                let html = plan.render_fragment_response(execution, fragment)?;
                let status = self.response_status(StatusCode::OK);
                LiveResponseComposition::html(status, self.compose_html_graph(html))
            }
            HandlerResponse::Redirect(redirect) => {
                let status = StatusCode::from_u16(redirect.status).unwrap_or(StatusCode::SEE_OTHER);
                LiveResponseComposition::redirect(status, redirect.location.clone())
            }
            HandlerResponse::Json(json) => {
                let payload = self.compose_json_payload(json.payload.clone());
                let status = self
                    .response_status(StatusCode::from_u16(json.status).unwrap_or(StatusCode::OK));
                LiveResponseComposition::json(status, payload)
            }
            HandlerResponse::File(file) => LiveResponseComposition::file(
                StatusCode::OK,
                file.logical_path.clone(),
                file.content_type.clone(),
                file.delivery_mode,
            ),
        };

        response = response.with_annotation(annotations);

        for cookie in execution.response_cookies.clone() {
            response = response.with_cookie(cookie);
        }

        Ok(response)
    }

    fn compose_html_graph(&self, html: String) -> LiveHtmlResponseGraph {
        let mut graph = LiveHtmlResponseGraph::new(html);
        graph = graph.with_request_surface(self.request_surface_output());
        for receipt in &self.render_hooks {
            graph = graph.with_render_hook(
                receipt.handler_id.to_string(),
                receipt.typed_output.as_ref(),
            );
        }
        for receipt in &self.admin_widgets {
            graph = graph.with_admin_widget(
                receipt.handler_id.to_string(),
                receipt.typed_output.as_ref(),
            );
        }
        graph
    }

    fn compose_json_payload(&self, json: BTreeMap<String, String>) -> BTreeMap<String, String> {
        let graph =
            LiveJsonResponseGraph::new(json).with_request_surface(self.request_surface_output());
        graph.render()
    }

    fn typed_outputs(&self) -> Vec<&TypedExecutionOutput> {
        let mut outputs = Vec::new();
        if let Some(receipt) = self.request_surface_output() {
            outputs.push(receipt);
        }
        for receipt in &self.render_hooks {
            if let Some(output) = receipt.typed_output.as_ref() {
                outputs.push(output);
            }
        }
        for receipt in &self.admin_widgets {
            if let Some(output) = receipt.typed_output.as_ref() {
                outputs.push(output);
            }
        }
        outputs
    }
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
