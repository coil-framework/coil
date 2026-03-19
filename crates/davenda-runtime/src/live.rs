use super::*;

mod execution;
mod graph;
mod response;

pub(crate) use execution::LiveExecutionReceipts;
pub(crate) use graph::LiveHtmlResponseGraph;
pub(crate) use response::{LiveCacheHeaders, LiveResponseAnnotations, LiveResponseComposition};

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
