use std::collections::BTreeSet;

use crate::error::WasmModelError;
use crate::grants::HostCapabilityGrant;
use crate::ids::{ExtensionPointKind, HttpMethod};
use crate::validation::{validate_route, validate_token};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageExtensionPoint {
    pub route: String,
    pub methods: BTreeSet<HttpMethod>,
}

impl PageExtensionPoint {
    pub fn new(
        route: impl Into<String>,
        methods: impl IntoIterator<Item = HttpMethod>,
    ) -> Result<Self, WasmModelError> {
        let route = validate_route("page_route", route.into())?;
        let methods = methods.into_iter().collect::<BTreeSet<_>>();

        if methods.is_empty() {
            return Err(WasmModelError::EmptyField {
                field: "page_methods",
            });
        }

        for method in &methods {
            if !matches!(
                method,
                HttpMethod::Get | HttpMethod::Head | HttpMethod::Post
            ) {
                return Err(WasmModelError::UnsupportedPageMethod { method: *method });
            }
        }

        Ok(Self { route, methods })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiExtensionPoint {
    pub route: String,
    pub methods: BTreeSet<HttpMethod>,
}

impl ApiExtensionPoint {
    pub fn new(
        route: impl Into<String>,
        methods: impl IntoIterator<Item = HttpMethod>,
    ) -> Result<Self, WasmModelError> {
        let route = validate_route("api_route", route.into())?;
        let methods = methods.into_iter().collect::<BTreeSet<_>>();

        if methods.is_empty() {
            return Err(WasmModelError::EmptyField {
                field: "api_methods",
            });
        }

        Ok(Self { route, methods })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExtensionPoint {
    pub job_name: String,
    pub queue: String,
}

impl JobExtensionPoint {
    pub fn new(
        job_name: impl Into<String>,
        queue: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            job_name: validate_token("job_name", job_name.into())?,
            queue: validate_token("queue", queue.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledJobExtensionPoint {
    pub job_name: String,
    pub schedule: String,
}

impl ScheduledJobExtensionPoint {
    pub fn new(
        job_name: impl Into<String>,
        schedule: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            job_name: validate_token("scheduled_job_name", job_name.into())?,
            schedule: crate::validation::require_non_empty("schedule", schedule.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookExtensionPoint {
    pub source: String,
    pub event: String,
}

impl WebhookExtensionPoint {
    pub fn new(
        source: impl Into<String>,
        event: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            source: validate_token("webhook_source", source.into())?,
            event: validate_token("webhook_event", event.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminWidgetExtensionPoint {
    pub slot: String,
}

impl AdminWidgetExtensionPoint {
    pub fn new(slot: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            slot: validate_token("admin_widget_slot", slot.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderHookExtensionPoint {
    pub slot: String,
}

impl RenderHookExtensionPoint {
    pub fn new(slot: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            slot: validate_token("render_hook_slot", slot.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionPoint {
    Page(PageExtensionPoint),
    Api(ApiExtensionPoint),
    Job(JobExtensionPoint),
    ScheduledJob(ScheduledJobExtensionPoint),
    Webhook(WebhookExtensionPoint),
    AdminWidget(AdminWidgetExtensionPoint),
    RenderHook(RenderHookExtensionPoint),
}

impl ExtensionPoint {
    pub fn kind(&self) -> ExtensionPointKind {
        match self {
            Self::Page(_) => ExtensionPointKind::Page,
            Self::Api(_) => ExtensionPointKind::Api,
            Self::Job(_) => ExtensionPointKind::Job,
            Self::ScheduledJob(_) => ExtensionPointKind::ScheduledJob,
            Self::Webhook(_) => ExtensionPointKind::Webhook,
            Self::AdminWidget(_) => ExtensionPointKind::AdminWidget,
            Self::RenderHook(_) => ExtensionPointKind::RenderHook,
        }
    }

    pub(crate) fn supports_grant(&self, grant: &HostCapabilityGrant) -> bool {
        match grant {
            HostCapabilityGrant::RenderFragment { .. } => {
                matches!(
                    self,
                    Self::Page(_) | Self::AdminWidget(_) | Self::RenderHook(_)
                )
            }
            HostCapabilityGrant::MetadataWrite { .. } | HostCapabilityGrant::CacheHintWrite => {
                matches!(
                    self,
                    Self::Page(_) | Self::Api(_) | Self::AdminWidget(_) | Self::RenderHook(_)
                )
            }
            _ => true,
        }
    }
}
