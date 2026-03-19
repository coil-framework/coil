use super::*;

pub(crate) fn module_http_contributions(
    manifests: &[ModuleManifest],
) -> Result<(Vec<RouteDefinition>, Vec<HandlerDefinition>), RouteBuildError> {
    let mut routes = Vec::new();
    let mut handlers = Vec::new();

    for manifest in manifests {
        for surface in &manifest.http_surfaces {
            routes.push(route_definition_from_surface(&manifest.name, surface)?);
            handlers.push(handler_definition_from_surface(surface)?);
        }
    }

    Ok((routes, handlers))
}

pub(crate) fn build_http_runtime_plan<P>(
    package: &P,
    routes: &[RouteDefinition],
) -> Result<HttpRuntimePlan, RouteBuildError>
where
    P: AuthModelPackage,
{
    let mut seen = std::collections::BTreeSet::new();
    for route in routes {
        if !seen.insert((route.name.clone(), route.method)) {
            return Err(RouteBuildError::DuplicateRoute {
                name: route.name.clone(),
                method: route.method,
            });
        }

        if let RouteAuthGate::Capability(capability) = route.auth {
            if package.binding_for(capability).is_none() {
                return Err(RouteBuildError::MissingCapabilityBinding {
                    name: route.name.clone(),
                    capability,
                });
            }
        }
    }

    Ok(HttpRuntimePlan {
        middleware: vec![
            MiddlewareStage::TransportNormalization,
            MiddlewareStage::CustomerAppResolution,
            MiddlewareStage::TraceContext,
            MiddlewareStage::LocaleResolution,
            MiddlewareStage::SessionResolution,
            MiddlewareStage::BrowserPolicy,
            MiddlewareStage::ResponsePolicy,
        ],
        routes: routes.to_vec(),
    })
}

fn route_definition_from_surface(
    module: &str,
    surface: &HttpSurfaceContribution,
) -> Result<RouteDefinition, RouteBuildError> {
    let mut route = RouteDefinition::new(
        surface.name.clone(),
        http_method_from_surface(surface.method),
        surface.path.clone(),
    )?
    .from_module(module.to_string());

    route = match surface.area {
        HttpSurfaceArea::Public => route,
        HttpSurfaceArea::Account => route.with_area(RouteArea::Account),
        HttpSurfaceArea::Admin => route.with_area(RouteArea::Admin),
        HttpSurfaceArea::Api => route.with_area(RouteArea::Api),
        HttpSurfaceArea::Fragment => route.with_area(RouteArea::Fragment),
    };

    if surface.localized {
        route = route.localized();
    }

    route = match surface.capability {
        Some(capability) => route.requiring_capability(capability),
        None if surface.area == HttpSurfaceArea::Account => route.requiring_session(),
        None if surface.area == HttpSurfaceArea::Admin => route.requiring_session(),
        None => route,
    };

    Ok(route)
}

fn handler_definition_from_surface(
    surface: &HttpSurfaceContribution,
) -> Result<HandlerDefinition, RouteBuildError> {
    match &surface.response {
        HttpResponseContract::Page { template, status } => {
            let mut handler = HandlerDefinition::page(surface.name.clone(), template.clone())?;
            if let HandlerResponse::Page(page) = &mut handler.response {
                page.status = *status;
            }
            Ok(handler)
        }
        HttpResponseContract::Fragment {
            template,
            fragment_id,
        } => {
            HandlerDefinition::fragment(surface.name.clone(), template.clone(), fragment_id.clone())
        }
        HttpResponseContract::Redirect { location, status } => {
            let mut handler = HandlerDefinition::redirect(surface.name.clone(), location.clone())?;
            if let HandlerResponse::Redirect(redirect) = &mut handler.response {
                redirect.status = *status;
            }
            Ok(handler)
        }
        HttpResponseContract::Json { status, payload } => {
            let mut handler = HandlerDefinition::json(surface.name.clone(), payload.clone())?;
            if let HandlerResponse::Json(json) = &mut handler.response {
                json.status = *status;
            }
            Ok(handler)
        }
        HttpResponseContract::File {
            logical_path,
            content_type,
            delivery_mode,
        } => HandlerDefinition::file(
            surface.name.clone(),
            logical_path.clone(),
            content_type.clone(),
            file_delivery_mode_from_surface(*delivery_mode),
        ),
    }
}

fn http_method_from_surface(method: HttpSurfaceMethod) -> HttpMethod {
    match method {
        HttpSurfaceMethod::Get => HttpMethod::Get,
        HttpSurfaceMethod::Head => HttpMethod::Head,
        HttpSurfaceMethod::Post => HttpMethod::Post,
        HttpSurfaceMethod::Put => HttpMethod::Put,
        HttpSurfaceMethod::Patch => HttpMethod::Patch,
        HttpSurfaceMethod::Delete => HttpMethod::Delete,
    }
}

fn file_delivery_mode_from_surface(mode: HttpFileDeliveryMode) -> FileDeliveryMode {
    match mode {
        HttpFileDeliveryMode::PublicCdn => FileDeliveryMode::PublicCdn,
        HttpFileDeliveryMode::SignedUrl => FileDeliveryMode::SignedUrl,
        HttpFileDeliveryMode::AppProxy => FileDeliveryMode::AppProxy,
        HttpFileDeliveryMode::LocalOnly => FileDeliveryMode::LocalOnly,
    }
}
