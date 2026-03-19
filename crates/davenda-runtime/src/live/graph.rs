use std::collections::BTreeMap;

use davenda_wasm::{TypedExecutionOutput, TypedResponseBody};

#[derive(Debug, Clone)]
pub(crate) struct LiveHtmlResponseGraph {
    base_html: String,
    contributions: Vec<LiveHtmlContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LiveHtmlFragmentSource {
    RequestSurface,
    RenderHook { handler_id: String },
    AdminWidget { handler_id: String },
}

#[derive(Debug, Clone)]
pub(crate) struct LiveJsonResponseGraph {
    payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LiveHtmlContribution {
    source: LiveHtmlFragmentSource,
    markup: String,
}

impl LiveHtmlContribution {
    fn new(source: LiveHtmlFragmentSource, markup: impl Into<String>) -> Self {
        Self {
            source,
            markup: markup.into(),
        }
    }
}

impl LiveHtmlResponseGraph {
    pub(crate) fn new(base_html: impl Into<String>) -> Self {
        Self {
            base_html: base_html.into(),
            contributions: Vec::new(),
        }
    }

    pub(crate) fn with_request_surface(mut self, output: Option<&TypedExecutionOutput>) -> Self {
        if let Some(output) = output {
            self.push_output(LiveHtmlFragmentSource::RequestSurface, output);
        }
        self
    }

    pub(crate) fn with_output(
        mut self,
        source: LiveHtmlFragmentSource,
        output: &TypedExecutionOutput,
    ) -> Self {
        self.push_output(source, output);
        self
    }

    pub(crate) fn with_render_hook(
        self,
        handler_id: impl Into<String>,
        output: Option<&TypedExecutionOutput>,
    ) -> Self {
        match output {
            Some(output) => self.with_output(
                LiveHtmlFragmentSource::RenderHook {
                    handler_id: handler_id.into(),
                },
                output,
            ),
            None => self,
        }
    }

    pub(crate) fn with_admin_widget(
        self,
        handler_id: impl Into<String>,
        output: Option<&TypedExecutionOutput>,
    ) -> Self {
        match output {
            Some(output) => self.with_output(
                LiveHtmlFragmentSource::AdminWidget {
                    handler_id: handler_id.into(),
                },
                output,
            ),
            None => self,
        }
    }

    fn push_output(&mut self, source: LiveHtmlFragmentSource, output: &TypedExecutionOutput) {
        match &output.body {
            TypedResponseBody::HtmlDocument(html) => {
                self.contributions.push(LiveHtmlContribution::new(
                    source,
                    document_body_fragment(html),
                ));
            }
            TypedResponseBody::HtmlFragment(html) => self
                .contributions
                .push(LiveHtmlContribution::new(source, html.clone())),
            TypedResponseBody::JsonObject(_) => {}
        }
    }

    pub(crate) fn render(self) -> String {
        if self.contributions.is_empty() {
            return self.base_html;
        }

        let body_markup = self
            .contributions
            .into_iter()
            .map(|LiveHtmlContribution { source, markup }| {
                let _ = source;
                markup
            })
            .collect::<Vec<_>>()
            .join("");

        inject_body_markup(self.base_html, &body_markup)
    }
}

impl LiveJsonResponseGraph {
    pub(crate) fn new(payload: BTreeMap<String, String>) -> Self {
        Self { payload }
    }

    pub(crate) fn with_request_surface(mut self, output: Option<&TypedExecutionOutput>) -> Self {
        if let Some(output) = output {
            if let TypedResponseBody::JsonObject(typed_payload) = &output.body {
                self.payload.extend(typed_payload.clone());
            }
        }
        self
    }

    pub(crate) fn render(self) -> BTreeMap<String, String> {
        self.payload
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use davenda_wasm::{TypedExecutionOutput, TypedMetadata, TypedResponseBody};

    #[test]
    fn html_response_graph_injects_contributions_into_document_body() {
        let graph = LiveHtmlResponseGraph::new("<html><body><main>base</main></body></html>")
            .with_output(
                LiveHtmlFragmentSource::RequestSurface,
                &TypedExecutionOutput {
                    surface: davenda_wasm::ExtensionPointKind::Page,
                    status: 200,
                    body: TypedResponseBody::HtmlDocument(
                        "<html><body><section>page</section></body></html>".to_string(),
                    ),
                    metadata: TypedMetadata::new(),
                    cache_hint: None,
                },
            )
            .with_render_hook(
                "hook-1",
                Some(&TypedExecutionOutput {
                    surface: davenda_wasm::ExtensionPointKind::RenderHook,
                    status: 200,
                    body: TypedResponseBody::HtmlFragment("<aside>hook</aside>".to_string()),
                    metadata: TypedMetadata::new(),
                    cache_hint: None,
                }),
            )
            .with_admin_widget(
                "widget-1",
                Some(&TypedExecutionOutput {
                    surface: davenda_wasm::ExtensionPointKind::AdminWidget,
                    status: 200,
                    body: TypedResponseBody::HtmlFragment("<div>widget</div>".to_string()),
                    metadata: TypedMetadata::new(),
                    cache_hint: None,
                }),
            );

        assert_eq!(
            graph.render(),
            "<html><body><main>base</main><section>page</section><aside>hook</aside><div>widget</div></body></html>"
        );
    }

    #[test]
    fn html_response_graph_appends_when_document_has_no_body_tag() {
        let graph = LiveHtmlResponseGraph::new("<section>base</section>").with_output(
            LiveHtmlFragmentSource::RequestSurface,
            &TypedExecutionOutput {
                surface: davenda_wasm::ExtensionPointKind::Page,
                status: 200,
                body: TypedResponseBody::HtmlFragment("<aside>fragment</aside>".to_string()),
                metadata: TypedMetadata::new(),
                cache_hint: None,
            },
        );

        assert_eq!(
            graph.render(),
            "<section>base</section><aside>fragment</aside>"
        );
    }

    #[test]
    fn json_response_graph_merges_the_request_surface_payload() {
        let graph =
            LiveJsonResponseGraph::new(BTreeMap::from([("base".to_string(), "true".to_string())]))
                .with_request_surface(Some(&TypedExecutionOutput {
                    surface: davenda_wasm::ExtensionPointKind::Api,
                    status: 200,
                    body: TypedResponseBody::JsonObject(BTreeMap::from([
                        ("request".to_string(), "yes".to_string()),
                        ("base".to_string(), "override".to_string()),
                    ])),
                    metadata: TypedMetadata::new(),
                    cache_hint: None,
                }));

        assert_eq!(
            graph.render(),
            BTreeMap::from([
                ("base".to_string(), "override".to_string()),
                ("request".to_string(), "yes".to_string()),
            ])
        );
    }
}
