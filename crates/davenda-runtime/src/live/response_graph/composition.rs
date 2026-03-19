use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::Response;

use crate::FileDeliveryMode;
use crate::live::LiveHtmlResponseGraph;

use super::LiveResponseAnnotations;
use super::headers::{body_response, file_delivery_mode_name, render_json_object};

#[derive(Debug, Clone)]
pub(crate) struct LiveResponseGraph {
    status: StatusCode,
    body: LiveResponseBodyGraph,
    annotations: LiveResponseAnnotations,
    cookies: Vec<String>,
}

#[derive(Debug, Clone)]
enum LiveResponseBodyGraph {
    Html(LiveHtmlResponseGraph),
    Json(BTreeMap<String, String>),
    Redirect { location: String },
    File {
        logical_path: String,
        content_type: String,
        delivery_mode: FileDeliveryMode,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct LiveResponseComposition {
    graph: LiveResponseGraph,
}

impl LiveResponseGraph {
    fn new(status: StatusCode, body: LiveResponseBodyGraph) -> Self {
        Self {
            status,
            body,
            annotations: LiveResponseAnnotations::default(),
            cookies: Vec::new(),
        }
    }

    fn into_response(self) -> Response<Body> {
        let mut response = match self.body {
            LiveResponseBodyGraph::Html(body) => {
                body_response(self.status, body.render(), Some("text/html; charset=utf-8"))
            }
            LiveResponseBodyGraph::Json(payload) => {
                let body = render_json_object(payload);
                body_response(self.status, body, Some("application/json"))
            }
            LiveResponseBodyGraph::Redirect { location } => {
                let mut response = Response::new(Body::empty());
                *response.status_mut() = self.status;
                response.headers_mut().insert(
                    HeaderName::from_static("location"),
                    HeaderValue::from_str(&location)
                        .expect("redirect location is a valid header value"),
                );
                response
            }
            LiveResponseBodyGraph::File {
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

impl LiveResponseComposition {
    pub(crate) fn html(status: StatusCode, body: LiveHtmlResponseGraph) -> Self {
        Self {
            graph: LiveResponseGraph::new(status, LiveResponseBodyGraph::Html(body)),
        }
    }

    pub(crate) fn json(status: StatusCode, body: BTreeMap<String, String>) -> Self {
        Self {
            graph: LiveResponseGraph::new(status, LiveResponseBodyGraph::Json(body)),
        }
    }

    pub(crate) fn redirect(status: StatusCode, location: impl Into<String>) -> Self {
        Self {
            graph: LiveResponseGraph::new(
                status,
                LiveResponseBodyGraph::Redirect {
                    location: location.into(),
                },
            ),
        }
    }

    pub(crate) fn file(
        status: StatusCode,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: FileDeliveryMode,
    ) -> Self {
        Self {
            graph: LiveResponseGraph::new(
                status,
                LiveResponseBodyGraph::File {
                    logical_path: logical_path.into(),
                    content_type: content_type.into(),
                    delivery_mode,
                },
            ),
        }
    }

    pub(crate) fn with_annotation(mut self, annotations: LiveResponseAnnotations) -> Self {
        self.graph.annotations = annotations;
        self
    }

    pub(crate) fn with_cookie(mut self, value: impl Into<String>) -> Self {
        self.graph.cookies.push(value.into());
        self
    }

    pub(crate) fn into_response(self) -> Response<Body> {
        self.graph.into_response()
    }
}
