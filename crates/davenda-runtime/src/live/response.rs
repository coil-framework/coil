use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::response::Response;

#[derive(Debug, Clone)]
pub(crate) struct LiveResponseComposition {
    status: StatusCode,
    headers: HeaderMap,
    cookies: Vec<String>,
    body: LiveResponseBody,
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
        }
    }

    pub(crate) fn empty(status: StatusCode) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            cookies: Vec::new(),
            body: LiveResponseBody::Empty,
        }
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

    pub(crate) fn extend_headers(mut self, headers: HeaderMap) -> Self {
        for (name, value) in headers {
            if let Some(name) = name {
                self.headers.insert(name, value);
            }
        }
        self
    }

    pub(crate) fn extend_key_value_headers(mut self, headers: BTreeMap<String, String>) -> Self {
        for (name, value) in headers {
            self = self.with_header(name, value);
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
