use super::*;

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
    let subject =
        davenda_auth::DefaultSubject::entity(davenda_auth::Entity::user(principal_id.to_string()));
    let allowed = state
        .route_authorizer
        .check_capability(&subject, capability, &object)
        .await?;

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
