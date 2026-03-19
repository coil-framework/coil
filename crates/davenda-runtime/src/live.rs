use super::*;
use axum::http::{HeaderMap, HeaderName, HeaderValue};

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
