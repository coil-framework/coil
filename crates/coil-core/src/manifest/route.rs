use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteSurfaceKind {
    FrontendPage,
    FrontendAction,
    AdminPage,
    AdminAction,
    Api,
    Fragment,
    Asset,
    Webhook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSurface {
    pub name: String,
    pub kind: RouteSurfaceKind,
    pub path: String,
    pub localized: bool,
    pub capability: Option<Capability>,
}

impl RouteSurface {
    pub fn new(name: impl Into<String>, kind: RouteSurfaceKind, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            path: path.into(),
            localized: false,
            capability: None,
        }
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn gated_by(mut self, capability: Capability) -> Self {
        self.capability = Some(capability);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpSurfaceArea {
    Public,
    Account,
    Admin,
    Api,
    Fragment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpSurfaceMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpFileDeliveryMode {
    PublicCdn,
    SignedUrl,
    AppProxy,
    LocalOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpResponseContract {
    Page {
        template: String,
        status: u16,
    },
    Fragment {
        template: String,
        fragment_id: String,
    },
    Redirect {
        location: String,
        status: u16,
    },
    Json {
        status: u16,
        payload: BTreeMap<String, String>,
    },
    File {
        logical_path: String,
        content_type: String,
        delivery_mode: HttpFileDeliveryMode,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSurfaceContribution {
    pub name: String,
    pub method: HttpSurfaceMethod,
    pub path: String,
    pub area: HttpSurfaceArea,
    pub localized: bool,
    pub capability: Option<Capability>,
    pub response: HttpResponseContract,
}

impl HttpSurfaceContribution {
    pub fn page(
        name: impl Into<String>,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            method: HttpSurfaceMethod::Get,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::Page {
                template: template.into(),
                status: 200,
            },
        }
    }

    pub fn fragment(
        name: impl Into<String>,
        path: impl Into<String>,
        template: impl Into<String>,
        fragment_id: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            method: HttpSurfaceMethod::Get,
            path: path.into(),
            area: HttpSurfaceArea::Fragment,
            localized: false,
            capability: None,
            response: HttpResponseContract::Fragment {
                template: template.into(),
                fragment_id: fragment_id.into(),
            },
        }
    }

    pub fn json(
        name: impl Into<String>,
        method: HttpSurfaceMethod,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        status: u16,
        payload: BTreeMap<String, String>,
    ) -> Self {
        Self {
            name: name.into(),
            method,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::Json { status, payload },
        }
    }

    pub fn redirect(
        name: impl Into<String>,
        method: HttpSurfaceMethod,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        location: impl Into<String>,
        status: u16,
    ) -> Self {
        Self {
            name: name.into(),
            method,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::Redirect {
                location: location.into(),
                status,
            },
        }
    }

    pub fn file(
        name: impl Into<String>,
        area: HttpSurfaceArea,
        path: impl Into<String>,
        logical_path: impl Into<String>,
        content_type: impl Into<String>,
        delivery_mode: HttpFileDeliveryMode,
    ) -> Self {
        Self {
            name: name.into(),
            method: HttpSurfaceMethod::Get,
            path: path.into(),
            area,
            localized: false,
            capability: None,
            response: HttpResponseContract::File {
                logical_path: logical_path.into(),
                content_type: content_type.into(),
                delivery_mode,
            },
        }
    }

    pub fn localized(mut self) -> Self {
        self.localized = true;
        self
    }

    pub fn gated_by(mut self, capability: Capability) -> Self {
        self.capability = Some(capability);
        self
    }
}
