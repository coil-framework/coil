use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use crate::error::WasmModelError;
use crate::grants::{
    HostCapabilityGrant, HostGrantSet, MetadataGrant, ResourceLimits, StorageClassGrant,
};
use crate::host_services::HostServiceSessionState;
use crate::ids::{ExtensionId, ExtensionPointKind, HandlerId, HttpMethod};
use crate::output::TypedExecutionOutput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerAppContext {
    pub app_id: String,
    pub tenant_id: Option<String>,
    pub site_id: Option<String>,
    pub brand_id: Option<String>,
    pub locale: Option<String>,
}

impl CustomerAppContext {
    pub fn new(app_id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            app_id: crate::validation::validate_token("app_id", app_id.into())?,
            tenant_id: None,
            site_id: None,
            brand_id: None,
            locale: None,
        })
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.tenant_id = Some(crate::validation::validate_token(
            "tenant_id",
            tenant_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_site_id(mut self, site_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.site_id = Some(crate::validation::validate_token(
            "site_id",
            site_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_brand_id(mut self, brand_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.brand_id = Some(crate::validation::validate_token(
            "brand_id",
            brand_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Result<Self, WasmModelError> {
        self.locale = Some(crate::validation::validate_token("locale", locale.into())?);
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrincipalKind {
    Anonymous,
    User,
    ServiceAccount,
}

impl fmt::Display for PrincipalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anonymous => f.write_str("anonymous"),
            Self::User => f.write_str("user"),
            Self::ServiceAccount => f.write_str("service_account"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrincipalRef {
    pub kind: PrincipalKind,
    pub id: Option<String>,
}

impl PrincipalRef {
    pub fn anonymous() -> Self {
        Self {
            kind: PrincipalKind::Anonymous,
            id: None,
        }
    }

    pub fn user(id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            kind: PrincipalKind::User,
            id: Some(crate::validation::validate_token(
                "principal_id",
                id.into(),
            )?),
        })
    }

    pub fn service_account(id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            kind: PrincipalKind::ServiceAccount,
            id: Some(crate::validation::validate_token(
                "principal_id",
                id.into(),
            )?),
        })
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        match self.kind {
            PrincipalKind::Anonymous => Ok(()),
            PrincipalKind::User | PrincipalKind::ServiceAccount => {
                if self.id.as_deref().is_some_and(|id| !id.is_empty()) {
                    Ok(())
                } else {
                    Err(WasmModelError::PrincipalIdRequired { kind: self.kind })
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub request_id: Option<String>,
}

impl TraceContext {
    pub fn new(trace_id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            trace_id: crate::validation::validate_token("trace_id", trace_id.into())?,
            parent_span_id: None,
            request_id: None,
        })
    }

    pub fn with_parent_span_id(
        mut self,
        parent_span_id: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.parent_span_id = Some(crate::validation::validate_token(
            "parent_span_id",
            parent_span_id.into(),
        )?);
        Ok(self)
    }

    pub fn with_request_id(
        mut self,
        request_id: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.request_id = Some(crate::validation::validate_token(
            "request_id",
            request_id.into(),
        )?);
        Ok(self)
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationContext {
    pub customer_app: CustomerAppContext,
    pub principal: PrincipalRef,
    pub trace: TraceContext,
    pub extension_config: BTreeMap<String, crate::manifest::ExtensionConfigValue>,
    pub input: InvocationInput,
}

impl InvocationContext {
    pub fn new(
        customer_app: CustomerAppContext,
        principal: PrincipalRef,
        trace: TraceContext,
        input: InvocationInput,
    ) -> Self {
        Self {
            customer_app,
            principal,
            trace,
            extension_config: BTreeMap::new(),
            input,
        }
    }

    pub fn with_config_value(
        mut self,
        key: impl Into<String>,
        value: crate::manifest::ExtensionConfigValue,
    ) -> Result<Self, WasmModelError> {
        let key = crate::validation::validate_token("extension_config_key", key.into())?;
        value.validate_for_key(&key)?;
        self.extension_config.insert(key, value);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        self.principal.validate()?;
        for (key, value) in &self.extension_config {
            crate::validation::validate_token("extension_config_key", key.clone())?;
            value.validate_for_key(key)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationPlan {
    pub extension_id: ExtensionId,
    pub handler_id: HandlerId,
    pub point: ExtensionPointKind,
    pub customer_app_id: String,
    pub granted_capabilities: HostGrantSet,
    pub limits: ResourceLimits,
    pub context: InvocationContext,
}

impl InvocationPlan {
    pub fn begin_execution(self) -> WasmExecutionSession {
        WasmExecutionSession::new(self)
    }

    pub fn grant_slots(&self) -> Vec<HostCapabilityGrant> {
        self.granted_capabilities.iter().cloned().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCall {
    DataRead {
        resource: String,
    },
    DataWrite {
        resource: String,
    },
    AuthCheck,
    AuthList,
    AuthLookup,
    AuthTupleWrite,
    StorageRead {
        class: StorageClassGrant,
    },
    StorageWrite {
        class: StorageClassGrant,
        bytes: u64,
    },
    RenderFragment {
        slot: String,
    },
    MetadataWrite {
        kind: MetadataGrant,
    },
    CacheHintWrite,
    OutboundHttp {
        integration: String,
        response_bytes: u64,
    },
    SecretRead {
        secret: String,
    },
    EnqueueJob {
        queue: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationOutcome {
    Page,
    ApiJson,
    JobCompleted,
    ScheduledJobCompleted,
    WebhookAccepted,
    AdminWidget,
    RenderHook,
}

impl InvocationOutcome {
    pub fn engine_code(&self) -> i32 {
        match self {
            Self::Page => 0,
            Self::ApiJson => 1,
            Self::JobCompleted => 2,
            Self::ScheduledJobCompleted => 3,
            Self::WebhookAccepted => 4,
            Self::AdminWidget => 5,
            Self::RenderHook => 6,
        }
    }

    pub fn from_engine_code(code: i32, handler_id: String) -> Result<Self, WasmModelError> {
        match code {
            0 => Ok(Self::Page),
            1 => Ok(Self::ApiJson),
            2 => Ok(Self::JobCompleted),
            3 => Ok(Self::ScheduledJobCompleted),
            4 => Ok(Self::WebhookAccepted),
            5 => Ok(Self::AdminWidget),
            6 => Ok(Self::RenderHook),
            _ => Err(WasmModelError::InvalidOutcomeCode { handler_id, code }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExecutionUsage {
    pub outbound_requests: u32,
    pub outbound_response_bytes: u64,
    pub storage_writes: u32,
    pub storage_bytes: u64,
    pub peak_concurrency: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    pub extension_id: ExtensionId,
    pub handler_id: HandlerId,
    pub point: ExtensionPointKind,
    pub runtime: Duration,
    pub usage: ExecutionUsage,
    pub outcome: InvocationOutcome,
    pub host_calls: Vec<HostCall>,
    pub typed_output: Option<TypedExecutionOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExecutionSession {
    state: HostServiceSessionState,
}

impl WasmExecutionSession {
    pub fn new(plan: InvocationPlan) -> Self {
        Self {
            state: HostServiceSessionState::new(plan),
        }
    }

    pub fn plan(&self) -> &InvocationPlan {
        self.state.plan()
    }

    pub fn usage(&self) -> &ExecutionUsage {
        self.state.usage()
    }

    pub fn host_calls(&self) -> &[HostCall] {
        self.state.host_calls()
    }

    pub fn grant_slots(&self) -> Vec<HostCapabilityGrant> {
        self.state.grant_slots()
    }

    pub fn record_host_call(&mut self, call: HostCall) -> Result<(), WasmModelError> {
        self.state.record_host_call(call)
    }

    pub fn reserve_concurrency(&mut self, units: u16) -> Result<(), WasmModelError> {
        self.state.reserve_concurrency(units)
    }

    pub fn release_concurrency(&mut self, units: u16) {
        self.state.release_concurrency(units)
    }

    pub fn finish(
        self,
        runtime: Duration,
        outcome: InvocationOutcome,
        typed_output: Option<TypedExecutionOutput>,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        self.state.finish(runtime, outcome, typed_output)
    }
}
