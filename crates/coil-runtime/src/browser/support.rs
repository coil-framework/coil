use super::*;
use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const FLASH_COOKIE_MAX_AGE_SECS: u64 = 300;
static SESSION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) fn validate_browser_value(
    field: &'static str,
    value: String,
) -> Result<String, RuntimeBrowserError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RuntimeBrowserError::EmptyValue { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(super) fn issue_session_id() -> String {
    let sequence = SESSION_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    let mut output = String::with_capacity(40);
    let _ = write!(output, "{nanos:032x}{sequence:08x}");
    output
}

pub(super) fn map_session_cookie_error(error: BrowserSecurityError) -> RuntimeBrowserError {
    RuntimeBrowserError::InvalidSessionCookie {
        reason: error.to_string(),
    }
}

pub(super) fn map_flash_cookie_error(error: BrowserSecurityError) -> RuntimeBrowserError {
    RuntimeBrowserError::InvalidFlashCookie {
        reason: error.to_string(),
    }
}
