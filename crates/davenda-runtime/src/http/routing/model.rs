use std::collections::BTreeMap;

use davenda_config::PlatformConfig;
use davenda_core::ModuleManifest;

use super::error::{
    RouteBuildError, RouteUrlError, validate_host, validate_route_name, validate_route_path,
};
use super::matching::{match_route_path, render_route_path};

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
        let site = config.site_for_host(host);
        let site_id = site.map(|site| site.id.clone());
        let default_locale = config.default_locale_for_site(site_id.as_deref());
        let supported_locales = site
            .map(|site| site.supported_locales.as_slice())
            .unwrap_or(config.i18n.supported_locales.as_slice());
        let localized_routes = site
            .and_then(|site| site.localized_routes)
            .unwrap_or(config.i18n.localized_routes);
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
                            site_id: site_id.clone(),
                            locale: None,
                            auth: route.auth,
                            params,
                        },
                    }),
                    None => None,
                },
                LocalePolicy::Localized if localized_routes => {
                    if route.path == "/" && path == "/" {
                        return Some(ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                site_id: site_id.clone(),
                                locale: Some(default_locale.to_string()),
                                auth: route.auth,
                                params: BTreeMap::new(),
                            },
                        });
                    }

                    supported_locales.iter().find_map(|locale| {
                        if route.path == "/" {
                            let localized_root = format!("/{}", locale.trim_matches('/'));
                            if path == localized_root || path == format!("{localized_root}/") {
                                return Some(ResolvedRouteMatch {
                                    route: route.clone(),
                                    resolved: ResolvedRoute {
                                        route_name: route.name.clone(),
                                        site_id: site_id.clone(),
                                        locale: Some(locale.clone()),
                                        auth: route.auth,
                                        params: BTreeMap::new(),
                                    },
                                });
                            }
                            return None;
                        }

                        let localized_path = format!(
                            "/{}/{}",
                            locale.trim_matches('/'),
                            route.path.trim_start_matches('/')
                        );
                        match_route_path(&localized_path, path).map(|params| ResolvedRouteMatch {
                            route: route.clone(),
                            resolved: ResolvedRoute {
                                route_name: route.name.clone(),
                                site_id: site_id.clone(),
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
        self.path_for_site(config, None, route_name, params, locale)
    }

    pub fn path_for_site(
        &self,
        config: &PlatformConfig,
        site_id: Option<&str>,
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
            let locale = locale.unwrap_or(config.default_locale_for_site(site_id));
            if !config
                .supported_locales_for_site(site_id)
                .iter()
                .any(|item| item == locale)
            {
                return Err(RouteUrlError::UnsupportedLocale {
                    route: route_name.to_string(),
                    locale: locale.to_string(),
                });
            }

            if rendered_path == "/" {
                if locale == config.default_locale_for_site(site_id) {
                    return Ok("/".to_string());
                }
                return Ok(format!("/{}", locale.trim_matches('/')));
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
        self.absolute_url_for_site(config, None, route_name, params, locale)
    }

    pub fn absolute_url_for_site(
        &self,
        config: &PlatformConfig,
        site_id: Option<&str>,
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
        let path = self.path_for_site(config, site_id, route_name, params, locale)?;
        let host = match &route.host {
            HostPattern::Exact(host) => host.as_str(),
            HostPattern::Any => config.canonical_host_for_site(site_id),
        };
        Ok(format!("https://{host}{path}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRoute {
    pub route_name: String,
    pub site_id: Option<String>,
    pub locale: Option<String>,
    pub auth: RouteAuthGate,
    pub params: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRouteMatch {
    pub route: RouteDefinition,
    pub resolved: ResolvedRoute,
}

impl ResolvedRoute {
    pub fn capability_auth_resource<P>(
        &self,
        route: &RouteDefinition,
        module_manifest: Option<&ModuleManifest>,
        package: &P,
    ) -> Result<Option<davenda_auth::Entity>, davenda_auth::DavendaAuthError>
    where
        P: davenda_auth::AuthModelPackage + ?Sized,
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

        Ok(Some(super::resolution::route_capability_resource(
            namespace,
            route.module.as_deref(),
            contract_kind,
            &self.route_name,
        )))
    }
}
