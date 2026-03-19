use crate::error::WasmModelError;
use crate::ids::{ExtensionPointKind, HttpMethod};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageInvocation {
    pub route: String,
    pub method: HttpMethod,
}

impl PageInvocation {
    pub fn new(route: impl Into<String>, method: HttpMethod) -> Result<Self, WasmModelError> {
        Ok(Self {
            route: crate::validation::validate_route("page_invocation_route", route.into())?,
            method,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiInvocation {
    pub route: String,
    pub method: HttpMethod,
}

impl ApiInvocation {
    pub fn new(route: impl Into<String>, method: HttpMethod) -> Result<Self, WasmModelError> {
        Ok(Self {
            route: crate::validation::validate_route("api_invocation_route", route.into())?,
            method,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobInvocation {
    pub job_name: String,
    pub attempt: u32,
}

impl JobInvocation {
    pub fn new(job_name: impl Into<String>, attempt: u32) -> Result<Self, WasmModelError> {
        Ok(Self {
            job_name: crate::validation::validate_token("job_invocation_name", job_name.into())?,
            attempt,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledJobInvocation {
    pub job_name: String,
}

impl ScheduledJobInvocation {
    pub fn new(job_name: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            job_name: crate::validation::validate_token(
                "scheduled_job_invocation_name",
                job_name.into(),
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookInvocation {
    pub source: String,
    pub event: String,
    pub verified: bool,
    pub replay_protected: bool,
}

impl WebhookInvocation {
    pub fn new(
        source: impl Into<String>,
        event: impl Into<String>,
        verified: bool,
        replay_protected: bool,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            source: crate::validation::validate_token("webhook_invocation_source", source.into())?,
            event: crate::validation::validate_token("webhook_invocation_event", event.into())?,
            verified,
            replay_protected,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminWidgetInvocation {
    pub slot: String,
}

impl AdminWidgetInvocation {
    pub fn new(slot: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            slot: crate::validation::validate_token("admin_widget_invocation_slot", slot.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderHookInvocation {
    pub slot: String,
}

impl RenderHookInvocation {
    pub fn new(slot: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            slot: crate::validation::validate_token("render_hook_invocation_slot", slot.into())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationInput {
    Page(PageInvocation),
    Api(ApiInvocation),
    Job(JobInvocation),
    ScheduledJob(ScheduledJobInvocation),
    Webhook(WebhookInvocation),
    AdminWidget(AdminWidgetInvocation),
    RenderHook(RenderHookInvocation),
}

impl InvocationInput {
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
}
