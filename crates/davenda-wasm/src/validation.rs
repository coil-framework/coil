use std::collections::BTreeMap;

use crate::error::WasmModelError;
use crate::ids::{ExtensionPointKind, HandlerId};
use crate::invocation::InvocationInput;
use crate::points::ExtensionPoint;

pub(crate) fn require_non_empty(
    field: &'static str,
    value: String,
) -> Result<String, WasmModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(WasmModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

pub(crate) fn validate_token(field: &'static str, value: String) -> Result<String, WasmModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(WasmModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn validate_sha256(
    field: &'static str,
    value: String,
) -> Result<String, WasmModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed.len() == 64
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    {
        Ok(trimmed)
    } else {
        Err(WasmModelError::InvalidChecksum {
            field,
            value: trimmed,
        })
    }
}

pub(crate) fn validate_route(field: &'static str, route: String) -> Result<String, WasmModelError> {
    let route = require_non_empty(field, route)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(WasmModelError::InvalidRoute { field, route })
    }
}

pub(crate) fn validate_invocation_target(
    handler_id: &HandlerId,
    point: &ExtensionPoint,
    input: &InvocationInput,
) -> Result<(), WasmModelError> {
    match (point, input) {
        (ExtensionPoint::Page(page), InvocationInput::Page(invocation)) => {
            if page.route != invocation.route {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "page route `{}` does not match registered route `{}`",
                        invocation.route, page.route
                    ),
                });
            }
            if !page.methods.contains(&invocation.method) {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "page method `{}` is not enabled for `{}`",
                        invocation.method, page.route
                    ),
                });
            }
        }
        (ExtensionPoint::Api(api), InvocationInput::Api(invocation)) => {
            if api.route != invocation.route {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "api route `{}` does not match registered route `{}`",
                        invocation.route, api.route
                    ),
                });
            }
            if !api.methods.contains(&invocation.method) {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "api method `{}` is not enabled for `{}`",
                        invocation.method, api.route
                    ),
                });
            }
        }
        (ExtensionPoint::Job(job), InvocationInput::Job(invocation)) => {
            if job.job_name != invocation.job_name {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "job `{}` does not match registered job `{}`",
                        invocation.job_name, job.job_name
                    ),
                });
            }
        }
        (ExtensionPoint::ScheduledJob(job), InvocationInput::ScheduledJob(invocation)) => {
            if job.job_name != invocation.job_name {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "scheduled job `{}` does not match registered job `{}`",
                        invocation.job_name, job.job_name
                    ),
                });
            }
        }
        (ExtensionPoint::Webhook(webhook), InvocationInput::Webhook(invocation)) => {
            if !invocation.verified {
                return Err(WasmModelError::UnverifiedWebhook {
                    handler_id: handler_id.to_string(),
                });
            }
            if !invocation.replay_protected {
                return Err(WasmModelError::ReplayUnsafeWebhook {
                    handler_id: handler_id.to_string(),
                });
            }
            if webhook.source != invocation.source || webhook.event != invocation.event {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "webhook `{}/{}` does not match registered `{}/{}`",
                        invocation.source, invocation.event, webhook.source, webhook.event
                    ),
                });
            }
        }
        (ExtensionPoint::AdminWidget(widget), InvocationInput::AdminWidget(invocation)) => {
            if widget.slot != invocation.slot {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "admin slot `{}` does not match registered slot `{}`",
                        invocation.slot, widget.slot
                    ),
                });
            }
        }
        (ExtensionPoint::RenderHook(hook), InvocationInput::RenderHook(invocation)) => {
            if hook.slot != invocation.slot {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "render slot `{}` does not match registered slot `{}`",
                        invocation.slot, hook.slot
                    ),
                });
            }
        }
        _ => {
            return Err(WasmModelError::InvocationPointMismatch {
                handler_id: handler_id.to_string(),
                expected: point.kind(),
                actual: input.kind(),
            });
        }
    }

    Ok(())
}

pub(crate) fn register_unique_target<K>(
    registry: &mut BTreeMap<K, crate::registry::RegisteredExtensionHandler>,
    key: K,
    binding: crate::registry::RegisteredExtensionHandler,
    target: String,
    point: ExtensionPointKind,
) -> Result<(), WasmModelError>
where
    K: Ord,
{
    if let Some(existing) = registry.insert(key, binding.clone()) {
        return Err(WasmModelError::DuplicateExtensionTarget {
            point,
            target,
            existing_handler: format!("{}::{}", existing.extension_id, existing.handler_id),
            conflicting_handler: format!("{}::{}", binding.extension_id, binding.handler_id),
        });
    }

    Ok(())
}
