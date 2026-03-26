use super::*;
use axum::extract::{Path, Query, State};
use axum::http::{
    HeaderValue, StatusCode,
    header::{CACHE_CONTROL, SET_COOKIE},
};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize, Default)]
struct DevLoginQuery {
    next: Option<String>,
}

pub(crate) fn development_router() -> Router<Arc<RuntimeServerState>> {
    Router::new()
        .route("/__dev", get(serve_dev_home))
        .route("/__dev/login/{persona}", get(issue_dev_session))
}

async fn serve_dev_home() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>Harbor Shop Local Dev</title>
  </head>
  <body>
    <main>
      <h1>Harbor Shop Local Dev</h1>
      <p>Use these shortcuts to enter the checked-in customer and operator journeys locally.</p>
      <ul>
        <li><a href="/__dev/login/customer?next=/account">Sign in as sample customer</a></li>
        <li><a href="/__dev/login/admin?next=/admin">Sign in as sample admin</a></li>
      </ul>
    </main>
  </body>
</html>
"#,
    )
}

async fn issue_dev_session(
    State(state): State<Arc<RuntimeServerState>>,
    Path(persona): Path<String>,
    Query(query): Query<DevLoginQuery>,
) -> Response {
    let (principal_id, default_target) = match persona.as_str() {
        "customer" => ("dev-customer", "/account"),
        "admin" => ("dev-admin", "/admin"),
        _ => {
            return (StatusCode::NOT_FOUND, "unknown local dev persona").into_response();
        }
    };
    let now = BrowserInstant::from_unix_seconds(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    let mut browser = state
        .browser
        .lock()
        .expect("runtime browser mutex poisoned");
    let request = match SessionIssueRequest::new()
        .for_principal(principal_id)
        .map_err(RequestExecutionError::from_browser_error)
        .map_err(RuntimeServerError::Execution)
    {
        Ok(request) => request,
        Err(error) => return error_response(error),
    };
    let issued = match browser
        .issue_session(request, &state.cookie_secret, now)
        .map_err(RequestExecutionError::from_browser_error)
        .map_err(RuntimeServerError::Execution)
    {
        Ok(issued) => issued,
        Err(error) => return error_response(error),
    };
    let target = query
        .next
        .as_deref()
        .filter(|target| target.starts_with('/'))
        .unwrap_or(default_target);
    let mut response = Redirect::temporary(target).into_response();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&issued.set_cookie_header)
            .expect("issued session cookies are valid header values"),
    );
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
}
