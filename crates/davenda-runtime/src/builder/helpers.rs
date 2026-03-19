use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static RUNTIME_PLAN_SCOPE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_runtime_plan_scope() -> String {
    format!(
        "runtime-plan:{}",
        RUNTIME_PLAN_SCOPE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

pub(crate) fn collect_extension_slots(
    manifests: &[ModuleManifest],
) -> Result<Vec<RegisteredExtensionSlot>, RuntimeBuildError> {
    let mut slots = Vec::new();
    let mut seen = BTreeMap::<(ExtensionPointKind, String), String>::new();

    for manifest in manifests {
        for slot in &manifest.extension_slots {
            let kind = extension_point_kind_for_slot(slot);
            let key = (kind, slot.surface.clone());
            if let Some(existing_module) = seen.insert(key.clone(), manifest.name.clone()) {
                return Err(RuntimeBuildError::DuplicateExtensionSlot {
                    kind,
                    surface: key.1,
                    first_module: existing_module,
                    second_module: manifest.name.clone(),
                });
            }

            slots.push(RegisteredExtensionSlot {
                module: manifest.name.clone(),
                kind,
                surface: slot.surface.clone(),
                description: slot.description.clone(),
            });
        }
    }

    Ok(slots)
}

pub(crate) fn collect_data_repositories(
    manifests: &[ModuleManifest],
) -> Result<Vec<RegisteredDataRepository>, RuntimeBuildError> {
    let mut repositories = Vec::new();
    let mut seen = BTreeMap::<String, String>::new();

    for manifest in manifests {
        for contribution in &manifest.data_repositories {
            if let Some(existing_module) =
                seen.insert(contribution.id.clone(), manifest.name.clone())
            {
                return Err(RuntimeBuildError::DuplicateDataRepository {
                    repository: contribution.id.clone(),
                    first_module: existing_module,
                    second_module: manifest.name.clone(),
                });
            }

            repositories.push(RegisteredDataRepository {
                module: manifest.name.clone(),
                contribution: contribution.clone(),
            });
        }
    }

    Ok(repositories)
}

pub(crate) fn validate_extension_handler_slot(
    handler: &davenda_wasm::RegisteredExtensionHandler,
    slots: &[RegisteredExtensionSlot],
) -> Result<(), RuntimeBuildError> {
    if slots
        .iter()
        .any(|slot| slot.kind == handler.point && slot.surface == handler.surface)
    {
        Ok(())
    } else {
        Err(RuntimeBuildError::UnknownExtensionSlot {
            extension_id: handler.extension_id.to_string(),
            handler_id: handler.handler_id.to_string(),
            point: handler.point,
            surface: handler.surface.clone(),
        })
    }
}

pub(crate) fn extension_point_kind_for_slot(
    slot: &davenda_core::ExtensionSlotDescriptor,
) -> ExtensionPointKind {
    match slot.kind {
        davenda_core::ExtensionSlotKind::Page => ExtensionPointKind::Page,
        davenda_core::ExtensionSlotKind::Api => ExtensionPointKind::Api,
        davenda_core::ExtensionSlotKind::Job => ExtensionPointKind::Job,
        davenda_core::ExtensionSlotKind::ScheduledJob => ExtensionPointKind::ScheduledJob,
        davenda_core::ExtensionSlotKind::Webhook => ExtensionPointKind::Webhook,
        davenda_core::ExtensionSlotKind::AdminWidget => ExtensionPointKind::AdminWidget,
        davenda_core::ExtensionSlotKind::RenderHook => ExtensionPointKind::RenderHook,
    }
}

pub(crate) fn build_handler_registry(
    routes: &[RouteDefinition],
    handlers: Vec<HandlerDefinition>,
) -> Result<BTreeMap<String, HandlerDefinition>, RuntimeBuildError> {
    let known_routes = routes
        .iter()
        .map(|route| route.name.as_str())
        .collect::<HashSet<_>>();
    let mut registry = BTreeMap::new();

    for handler in handlers {
        if !known_routes.contains(handler.route_name.as_str()) {
            return Err(RuntimeBuildError::UnknownHandlerRoute {
                route: handler.route_name,
            });
        }

        if registry
            .insert(handler.route_name.clone(), handler.clone())
            .is_some()
        {
            return Err(RuntimeBuildError::DuplicateHandler {
                route: handler.route_name,
            });
        }
    }

    Ok(registry)
}
