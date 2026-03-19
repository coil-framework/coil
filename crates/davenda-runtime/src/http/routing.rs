use davenda_config::PlatformConfig;
use davenda_core::ModuleManifest;
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub const fn is_state_changing(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch | Self::Delete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteArea {
    Public,
    Account,
    Admin,
    Api,
    Fragment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPattern {
    Any,
    Exact(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalePolicy {
    DefaultOnly,
    Localized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteAuthGate {
    Public,
    Session,
    Capability(davenda_auth::Capability),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDefinition {
    pub name: String,
    pub method: HttpMethod,
    pub path: String,
    pub area: RouteArea,
    pub host: HostPattern,
    pub locale_policy: LocalePolicy,
    pub auth: RouteAuthGate,
    pub module: Option<String>,
    pub feature_flag: Option<String>,
}

impl RouteDefinition {
    pub fn new(
        name: impl Into<String>,
        method: HttpMethod,
        path: impl Into<String>,
    ) -> Result<Self, RouteBuildError> {
        let name = validate_route_name(name.into())?;
        let path = validate_route_path(path.into())?;

        Ok(Self {
            name,
            method,
            path,
            area: RouteArea::Public,
            host: HostPattern::Any,
            locale_policy: LocalePolicy::DefaultOnly,
            auth: RouteAuthGate::Public,
            module: None,
            feature_flag: None,
        })
    }

    pub fn with_area(mut self, area: RouteArea) -> Self {
        self.area = area;
        self
    }

    pub fn with_host(mut self, host: impl Into<String>) -> Result<Self, RouteBuildError> {
        self.host = HostPattern::Exact(validate_host(host.into())?);
        Ok(self)
    }

    pub fn localized(mut self) -> Self {
        self.locale_policy = LocalePolicy::Localized;
        self
    }

    pub fn requiring_session(mut self) -> Self {
        self.auth = RouteAuthGate::Session;
        self
    }

    pub fn requiring_capability(mut self, capability: davenda_auth::Capability) -> Self {
        self.auth = RouteAuthGate::Capability(capability);
        self
    }

    pub fn from_module(mut self, module: impl Into<String>) -> Self {
        self.module = Some(module.into());
        self
    }

    pub fn with_feature_flag(mut self, feature_flag: impl Into<String>) -> Self {
        self.feature_flag = Some(feature_flag.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiddlewareStage {
    TransportNormalization,
    CustomerAppResolution,
    TraceContext,
    LocaleResolution,
    SessionResolution,
    BrowserPolicy,
    ResponsePolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRuntimePlan {
    pub middleware: Vec<MiddlewareStage>,
    pub routes: Vec<RouteDefinition>,
}

impl HttpRuntimePlan {
    pub fn resolve(
        &self,
        config: &PlatformConfig,
        method: HttpMethod,
        host: &str,
        path: &str,
    ) -> Option<ResolvedRoute> {
        self.resolve_match(config, method, host, path)
            .map(|matched| matched.resolved)
    }

    pub fn resolve_match(
        &self,
        config: &PlatformConfig,
        method: HttpMethod,
        host: &str,
        path: &str,
    ) -> Option<ResolvedRouteMatch> {
        self.routes.iter().find_map(|route| {
            if route.method != method {
                return None;
            }

            if let HostPattern::Exact(expected) = &route.host {
                if expected != host {
                    return None;
                }
            }

            match route.locale_policy {
                LocalePolicy::DefaultOnly => match match_route_path(&route.path, path) {
                    Some(params) => Some(ResolvedRouteMatch {
                        route: route.clone(),
                        resolved: ResolvedRoute {
                            route_name: route.name.clone(),
                            locale: None,
                            auth: route.auth,
                            params,
                        },
                    }),
                    None => None,
                },
                LocalePolicy::Localized if config.i18n.localized_routes => {
                    config.i18n.supported_locales.iter().find_map(|locale| {
                        let localized_path = format!(
                            "/{}/{}",
                            locale.trim_matches('/'),
                            route.path.trim_start_matches('/')
                        );
                        match_route_path(&localized_path, path).map(|params| ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                locale: Some(locale.clone()),
                                auth: route.auth,
                                params,
                            },
                        })
                    })
                }
                LocalePolicy::Localized => None,
            }
        })
    }

    pub fn path_for(
        &self,
        config: &PlatformConfig,
        route_name: &str,
        params: &BTreeMap<String, String>,
        locale: Option<&str>,
    ) -> Result<String, RouteUrlError> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| RouteUrlError::UnknownRoute {
                route: route_name.to_string(),
            })?;
        let rendered_path = render_route_path(&route.path, params, route_name)?;

        if route.locale_policy == LocalePolicy::Localized {
            let locale = locale.unwrap_or(&config.i18n.default_locale);
            if !config
                .i18n
                .supported_locales
                .iter()
                .any(|item| item == locale)
            {
                return Err(RouteUrlError::UnsupportedLocale {
                    route: route_name.to_string(),
                    locale: locale.to_string(),
                });
            }

            return Ok(format!(
                "/{}/{}",
                locale.trim_matches('/'),
                rendered_path.trim_start_matches('/')
            ));
        }

        Ok(rendered_path)
    }

    pub fn absolute_url_for(
        &self,
        config: &PlatformConfig,
        route_name: &str,
        params: &BTreeMap<String, String>,
        locale: Option<&str>,
    ) -> Result<String, RouteUrlError> {
        let route = self
            .routes
            .iter()
            .find(|route| route.name == route_name)
            .ok_or_else(|| RouteUrlError::UnknownRoute {
                route: route_name.to_string(),
            })?;
        let path = self.path_for(config, route_name, params, locale)?;
        let host = match &route.host {
            HostPattern::Exact(host) => host.as_str(),
            HostPattern::Any => config.seo.canonical_host.as_str(),
        };
        Ok(format!("https://{host}{path}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub route_name: String,
    pub locale: Option<String>,
    pub auth: RouteAuthGate,
    pub params: BTreeMap<String, String>,
}

impl ResolvedRoute {
    pub fn capability_auth_resource<P>(
        &self,
        route: &RouteDefinition,
        module_manifest: Option<&ModuleManifest>,
        package: &P,
    ) -> Result<Option<davenda_auth::Entity>, davenda_auth::DavendaAuthError>
    where
        P: davenda_auth::AuthModelPackage,
    {
        let RouteAuthGate::Capability(capability) = self.auth else {
            return Ok(None);
        };

        let binding = package
            .binding_for(capability)
            .ok_or(davenda_auth::DavendaAuthError::MissingCapabilityBinding { capability })?;
        let namespace = binding
            .resource_namespaces
            .first()
            .copied()
            .expect("route capability bindings must expose at least one namespace");
        let contract_kind = module_manifest
            .and_then(|manifest| {
                manifest
                    .capability_contracts
                    .iter()
                    .find(|contract| contract.capability == capability)
            })
            .and_then(|contract| contract.resource_kinds.first())
            .map(String::as_str);

        Ok(Some(route_capability_resource(
            namespace,
            route.module.as_deref(),
            contract_kind,
            &self.route_name,
        )))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRouteMatch {
    pub route: RouteDefinition,
    pub resolved: ResolvedRoute,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteBuildError {
    #[error("route names must not be empty")]
    EmptyRouteName,
    #[error("route paths must start with `/`, got `{path}`")]
    InvalidRoutePath { path: String },
    #[error("host pattern must not be empty")]
    EmptyHostPattern,
    #[error("route `{name}` is registered more than once for method {method:?}")]
    DuplicateRoute { name: String, method: HttpMethod },
    #[error(
        "route `{name}` requires capability `{capability}` but the auth package does not bind it"
    )]
    MissingCapabilityBinding {
        name: String,
        capability: davenda_auth::Capability,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RouteUrlError {
    #[error("route `{route}` is not registered")]
    UnknownRoute { route: String },
    #[error("route `{route}` requires parameter `{parameter}`")]
    MissingRouteParameter { route: String, parameter: String },
    #[error("route `{route}` does not support locale `{locale}`")]
    UnsupportedLocale { route: String, locale: String },
}

fn route_capability_resource(
    namespace: davenda_auth::Namespace,
    module: Option<&str>,
    contract_kind: Option<&str>,
    route_name: &str,
) -> davenda_auth::Entity {
    let resource_id = match (module, contract_kind) {
        (Some(module), Some(contract_kind)) => {
            format!("http.surface.module.{module}.{contract_kind}.{route_name}")
        }
        (Some(module), None) => format!("http.surface.module.{module}.{route_name}"),
        (None, Some(contract_kind)) => format!("http.surface.{contract_kind}.{route_name}"),
        (None, None) => format!("http.surface.{route_name}"),
    };

    match namespace {
        davenda_auth::Namespace::Tenant => davenda_auth::Entity::tenant(resource_id),
        davenda_auth::Namespace::Site => davenda_auth::Entity::site(resource_id),
        davenda_auth::Namespace::Brand => davenda_auth::Entity::brand(resource_id),
        davenda_auth::Namespace::Storefront => davenda_auth::Entity::storefront(resource_id),
        davenda_auth::Namespace::User => davenda_auth::Entity::user(resource_id),
        davenda_auth::Namespace::Group => davenda_auth::Entity::group(resource_id),
        davenda_auth::Namespace::Team => davenda_auth::Entity::team(resource_id),
        davenda_auth::Namespace::ServiceAccount => {
            davenda_auth::Entity::service_account(resource_id)
        }
        davenda_auth::Namespace::Page => davenda_auth::Entity::page(resource_id),
        davenda_auth::Namespace::Navigation => davenda_auth::Entity::navigation(resource_id),
        davenda_auth::Namespace::Product => davenda_auth::Entity::product(resource_id),
        davenda_auth::Namespace::Collection => davenda_auth::Entity::collection(resource_id),
        davenda_auth::Namespace::Order => davenda_auth::Entity::order(resource_id),
        davenda_auth::Namespace::Subscription => davenda_auth::Entity::subscription(resource_id),
        davenda_auth::Namespace::MembershipTier => {
            davenda_auth::Entity::membership_tier(resource_id)
        }
        davenda_auth::Namespace::Event => davenda_auth::Entity::event(resource_id),
        davenda_auth::Namespace::EventSlot => davenda_auth::Entity::event_slot(resource_id),
        davenda_auth::Namespace::Booking => davenda_auth::Entity::booking(resource_id),
        davenda_auth::Namespace::Media => davenda_auth::Entity::media(resource_id),
        davenda_auth::Namespace::MediaLibrary => davenda_auth::Entity::media_library(resource_id),
        davenda_auth::Namespace::Asset => davenda_auth::Entity::asset(resource_id),
        davenda_auth::Namespace::AssetFolder => davenda_auth::Entity::asset_folder(resource_id),
        davenda_auth::Namespace::ThemeAssetBundle => {
            davenda_auth::Entity::theme_asset_bundle(resource_id)
        }
        davenda_auth::Namespace::AdminModule => davenda_auth::Entity::admin_module(resource_id),
    }
}

fn match_route_path(pattern: &str, actual: &str) -> Option<BTreeMap<String, String>> {
    let pattern_segments = path_segments(pattern);
    let actual_segments = path_segments(actual);
    if pattern_segments.len() != actual_segments.len() {
        return None;
    }

    let mut params = BTreeMap::new();
    for (pattern_segment, actual_segment) in pattern_segments.iter().zip(actual_segments.iter()) {
        if pattern_segment.starts_with('{')
            && pattern_segment.ends_with('}')
            && pattern_segment.len() > 2
        {
            params.insert(
                pattern_segment[1..pattern_segment.len() - 1].to_string(),
                (*actual_segment).to_string(),
            );
        } else if pattern_segment != actual_segment {
            return None;
        }
    }

    Some(params)
}

fn path_segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn render_route_path(
    pattern: &str,
    params: &BTreeMap<String, String>,
    route_name: &str,
) -> Result<String, RouteUrlError> {
    let rendered_segments = path_segments(pattern)
        .into_iter()
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') && segment.len() > 2 {
                let parameter = &segment[1..segment.len() - 1];
                params
                    .get(parameter)
                    .cloned()
                    .ok_or_else(|| RouteUrlError::MissingRouteParameter {
                        route: route_name.to_string(),
                        parameter: parameter.to_string(),
                    })
            } else {
                Ok(segment.to_string())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    if rendered_segments.is_empty() {
        Ok("/".to_string())
    } else {
        Ok(format!("/{}", rendered_segments.join("/")))
    }
}

pub(super) fn validate_route_name(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyRouteName)
    } else {
        Ok(trimmed.to_string())
    }
}

pub(super) fn validate_route_path(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.starts_with('/') && !trimmed.is_empty() {
        Ok(trimmed.to_string())
    } else {
        Err(RouteBuildError::InvalidRoutePath {
            path: value.trim().to_string(),
        })
    }
}

pub(super) fn validate_host(value: String) -> Result<String, RouteBuildError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(RouteBuildError::EmptyHostPattern)
    } else {
        Ok(trimmed.to_string())
    }
}

pub(super) fn validate_template_name(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}

pub(super) fn validate_fragment_id(value: String) -> Result<String, RouteBuildError> {
    validate_route_name(value)
}
