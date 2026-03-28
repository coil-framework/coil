use super::*;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) async fn authorize_live_request(
    state: &RuntimeServerState,
    request: &mut RequestInput,
) -> Result<(), RuntimeServerError> {
    let matched = state
        .plan
        .http
        .resolve_match(
            &state.plan.config,
            request.method,
            &request.host,
            &request.path,
        )
        .ok_or_else(|| {
            RuntimeServerError::Execution(RequestExecutionError::RouteNotFound {
                method: request.method,
                host: request.host.clone(),
                path: request.path.clone(),
            })
        })?;

    let RouteAuthGate::Capability(capability) = matched.resolved.auth else {
        return Ok(());
    };
    if request.session_id.is_none() {
        return Ok(());
    }

    let Some(principal_id) = request.principal_id.as_deref() else {
        return Ok(());
    };
    if state.is_development() && principal_id == "dev-admin" {
        request.granted_capabilities.insert(capability);
        return Ok(());
    }
    let package = state.plan.auth_package.package();
    let module_manifest = matched.route.module.as_deref().and_then(|module_name| {
        state
            .plan
            .modules
            .iter()
            .find(|manifest| manifest.name == module_name)
    });
    let Some(object) = matched
        .resolved
        .capability_auth_resource(&matched.route, module_manifest, package)
        .map_err(|error| RuntimeServerError::Authorization {
            reason: error.to_string(),
        })?
    else {
        return Ok(());
    };
    let subject = match request.principal_kind {
        RequestPrincipalKind::ServiceAccount => coil_auth::DefaultSubject::entity(
            coil_auth::Entity::service_account(principal_id.to_string()),
        ),
        _ => coil_auth::DefaultSubject::entity(coil_auth::Entity::user(
            principal_id.to_string(),
        )),
    };
    let started_at = Instant::now();
    let allowed = state
        .route_authorizer
        .check_capability(&subject, capability, &object)
        .await?;
    let elapsed_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    let _ = state
        .plan
        .observability
        .telemetry
        .record_histogram("coil.auth.check.latency_ms", elapsed_ms);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = state.plan.observability.telemetry.record_trace(
        coil_observability::TraceRecord::new(
            format!("auth:{}:{}", matched.resolved.route_name, capability.as_str()),
            "auth.check",
            if allowed { "allowed" } else { "denied" },
            now,
        )
        .with_field("route", matched.resolved.route_name.clone())
        .with_field("capability", capability.as_str())
        .with_field("duration_ms", elapsed_ms.to_string()),
    );

    if allowed {
        request.granted_capabilities.insert(capability);
        Ok(())
    } else {
        Err(RuntimeServerError::Execution(
            RequestExecutionError::CapabilityRequired {
                route: matched.resolved.route_name.clone(),
                capability,
            },
        ))
    }
}
