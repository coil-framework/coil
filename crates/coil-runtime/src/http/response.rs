use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageResponse {
    pub template: String,
    pub status: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentResponse {
    pub template: String,
    pub fragment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedirectResponse {
    pub location: String,
    pub status: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonResponse {
    pub status: u16,
    pub payload: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileResponse {
    pub logical_path: String,
    pub content_type: String,
    pub delivery_mode: FileDeliveryMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandlerResponse {
    Page(PageResponse),
    Fragment(FragmentResponse),
    Redirect(RedirectResponse),
    Json(JsonResponse),
    File(FileResponse),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerDefinition {
    pub route_name: String,
    pub response: HandlerResponse,
}

impl HandlerDefinition {
    pub fn page(
        route_name: impl Into<String>,
        template: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Page(PageResponse {
                template: validate_template_name(template.into())?,
                status: 200,
            }),
        })
    }

    pub fn fragment(
        route_name: impl Into<String>,
        template: impl Into<String>,
        fragment_id: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Fragment(FragmentResponse {
                template: validate_template_name(template.into())?,
                fragment_id: validate_fragment_id(fragment_id.into())?,
            }),
        })
    }

    pub fn redirect(
        route_name: impl Into<String>,
        location: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Redirect(RedirectResponse {
                location: validate_route_path(location.into())?,
                status: 303,
            }),
        })
    }

    pub fn json(
        route_name: impl Into<String>,
        payload: BTreeMap<String, String>,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::Json(JsonResponse {
                status: 200,
                payload,
            }),
        })
    }

    pub fn file(
        route_name: impl Into<String>,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: FileDeliveryMode,
    ) -> Result<Self, RouteBuildError> {
        Ok(Self {
            route_name: validate_route_name(route_name.into())?,
            response: HandlerResponse::File(FileResponse {
                logical_path: validate_template_name(logical_path.into())?,
                content_type: validate_template_name(content_type.into())?,
                delivery_mode,
            }),
        })
    }
}
