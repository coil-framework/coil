use crate::error::WasmModelError;
use crate::ids::ExtensionPointKind;

use super::super::json_ld::RobotsDirective;
use super::TypedResponseBodyKind;

pub(super) fn robot_tag(directive: RobotsDirective) -> u8 {
    match directive {
        RobotsDirective::Index => 0,
        RobotsDirective::NoIndex => 1,
        RobotsDirective::Follow => 2,
        RobotsDirective::NoFollow => 3,
        RobotsDirective::NoArchive => 4,
    }
}

pub(super) fn robot_from_tag(tag: u8) -> Result<RobotsDirective, WasmModelError> {
    match tag {
        0 => Ok(RobotsDirective::Index),
        1 => Ok(RobotsDirective::NoIndex),
        2 => Ok(RobotsDirective::Follow),
        3 => Ok(RobotsDirective::NoFollow),
        4 => Ok(RobotsDirective::NoArchive),
        other => Err(WasmModelError::InvalidTypedReturn {
            reason: format!("typed robots directive tag `{other}` is invalid"),
        }),
    }
}

pub(super) fn extension_point_kind_from_tag(tag: u8) -> Result<ExtensionPointKind, WasmModelError> {
    match tag {
        0 => Ok(ExtensionPointKind::Page),
        1 => Ok(ExtensionPointKind::Api),
        2 => Ok(ExtensionPointKind::Job),
        3 => Ok(ExtensionPointKind::ScheduledJob),
        4 => Ok(ExtensionPointKind::Webhook),
        5 => Ok(ExtensionPointKind::AdminWidget),
        6 => Ok(ExtensionPointKind::RenderHook),
        other => Err(WasmModelError::InvalidTypedReturn {
            reason: format!("typed surface tag `{other}` is invalid"),
        }),
    }
}

pub(super) fn extension_point_kind_tag(kind: ExtensionPointKind) -> u8 {
    match kind {
        ExtensionPointKind::Page => 0,
        ExtensionPointKind::Api => 1,
        ExtensionPointKind::Job => 2,
        ExtensionPointKind::ScheduledJob => 3,
        ExtensionPointKind::Webhook => 4,
        ExtensionPointKind::AdminWidget => 5,
        ExtensionPointKind::RenderHook => 6,
    }
}

pub(super) fn expected_body_kind_for_point(
    point: ExtensionPointKind,
) -> Option<TypedResponseBodyKind> {
    match point {
        ExtensionPointKind::Page => Some(TypedResponseBodyKind::HtmlDocument),
        ExtensionPointKind::Api => Some(TypedResponseBodyKind::JsonObject),
        ExtensionPointKind::AdminWidget | ExtensionPointKind::RenderHook => {
            Some(TypedResponseBodyKind::HtmlFragment)
        }
        ExtensionPointKind::Job
        | ExtensionPointKind::ScheduledJob
        | ExtensionPointKind::Webhook => None,
    }
}

pub(super) fn validate_http_status(status: u16) -> Result<(), WasmModelError> {
    if (100..=599).contains(&status) {
        Ok(())
    } else {
        Err(WasmModelError::InvalidTypedStatus { status })
    }
}
