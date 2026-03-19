use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

use crate::error::WasmModelError;
use crate::grants::{
    HostCapabilityGrant, HostGrantSet, MetadataGrant, ResourceLimits, StorageClassGrant,
};
use crate::ids::{ExtensionId, ExtensionPointKind, HandlerId, HttpMethod};

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

impl HostCall {
    fn required_grant(&self) -> HostCapabilityGrant {
        match self {
            Self::DataRead { resource } => HostCapabilityGrant::DataRead {
                resource: resource.clone(),
            },
            Self::DataWrite { resource } => HostCapabilityGrant::DataWrite {
                resource: resource.clone(),
            },
            Self::AuthCheck => HostCapabilityGrant::AuthCheck,
            Self::AuthList => HostCapabilityGrant::AuthList,
            Self::AuthLookup => HostCapabilityGrant::AuthLookup,
            Self::AuthTupleWrite => HostCapabilityGrant::AuthTupleWrite,
            Self::StorageRead { class } => HostCapabilityGrant::StorageRead { class: *class },
            Self::StorageWrite { class, .. } => HostCapabilityGrant::StorageWrite { class: *class },
            Self::RenderFragment { slot } => {
                HostCapabilityGrant::RenderFragment { slot: slot.clone() }
            }
            Self::MetadataWrite { kind } => HostCapabilityGrant::MetadataWrite { kind: *kind },
            Self::CacheHintWrite => HostCapabilityGrant::CacheHintWrite,
            Self::OutboundHttp { integration, .. } => HostCapabilityGrant::OutboundHttp {
                integration: integration.clone(),
            },
            Self::SecretRead { secret } => HostCapabilityGrant::SecretRead {
                secret: secret.clone(),
            },
            Self::EnqueueJob { queue } => HostCapabilityGrant::EnqueueJob {
                queue: queue.clone(),
            },
        }
    }
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
    fn label(&self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::ApiJson => "api_json",
            Self::JobCompleted => "job_completed",
            Self::ScheduledJobCompleted => "scheduled_job_completed",
            Self::WebhookAccepted => "webhook_accepted",
            Self::AdminWidget => "admin_widget",
            Self::RenderHook => "render_hook",
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExecutionSession {
    plan: InvocationPlan,
    usage: ExecutionUsage,
    active_concurrency: u16,
}

impl WasmExecutionSession {
    pub fn new(plan: InvocationPlan) -> Self {
        Self {
            plan,
            usage: ExecutionUsage::default(),
            active_concurrency: 0,
        }
    }

    pub fn plan(&self) -> &InvocationPlan {
        &self.plan
    }

    pub fn usage(&self) -> &ExecutionUsage {
        &self.usage
    }

    pub fn record_host_call(&mut self, call: HostCall) -> Result<(), WasmModelError> {
        let grant = call.required_grant();
        if !self.plan.granted_capabilities.contains(&grant) {
            return Err(WasmModelError::HostGrantDenied {
                handler_id: self.plan.handler_id.to_string(),
                grant,
            });
        }

        match call {
            HostCall::StorageWrite { bytes, .. } => {
                self.usage.storage_writes = self.usage.storage_writes.saturating_add(1);
                self.usage.storage_bytes = self.usage.storage_bytes.saturating_add(bytes);
                if self.usage.storage_writes > self.plan.limits.max_storage_writes {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_storage_writes",
                    });
                }
                if self.usage.storage_bytes > self.plan.limits.max_storage_bytes {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_storage_bytes",
                    });
                }
            }
            HostCall::OutboundHttp { response_bytes, .. } => {
                self.usage.outbound_requests = self.usage.outbound_requests.saturating_add(1);
                self.usage.outbound_response_bytes = self
                    .usage
                    .outbound_response_bytes
                    .saturating_add(response_bytes);
                if self.usage.outbound_requests > self.plan.limits.max_outbound_requests {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_outbound_requests",
                    });
                }
                if self.usage.outbound_response_bytes > self.plan.limits.max_outbound_response_bytes
                {
                    return Err(WasmModelError::ResourceLimitExceeded {
                        handler_id: self.plan.handler_id.to_string(),
                        field: "max_outbound_response_bytes",
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn reserve_concurrency(&mut self, units: u16) -> Result<(), WasmModelError> {
        self.active_concurrency = self.active_concurrency.saturating_add(units);
        self.usage.peak_concurrency = self.usage.peak_concurrency.max(self.active_concurrency);
        if self.usage.peak_concurrency > self.plan.limits.max_concurrency {
            return Err(WasmModelError::ResourceLimitExceeded {
                handler_id: self.plan.handler_id.to_string(),
                field: "max_concurrency",
            });
        }
        Ok(())
    }

    pub fn release_concurrency(&mut self, units: u16) {
        self.active_concurrency = self.active_concurrency.saturating_sub(units);
    }

    pub fn finish(
        self,
        runtime: Duration,
        outcome: InvocationOutcome,
    ) -> Result<ExecutionReceipt, WasmModelError> {
        if runtime > self.plan.limits.max_runtime {
            return Err(WasmModelError::RuntimeBudgetExceeded {
                handler_id: self.plan.handler_id.to_string(),
                max_runtime: self.plan.limits.max_runtime,
                actual_runtime: runtime,
            });
        }

        let valid = matches!(
            (self.plan.point, &outcome),
            (ExtensionPointKind::Page, InvocationOutcome::Page)
                | (ExtensionPointKind::Api, InvocationOutcome::ApiJson)
                | (ExtensionPointKind::Job, InvocationOutcome::JobCompleted)
                | (
                    ExtensionPointKind::ScheduledJob,
                    InvocationOutcome::ScheduledJobCompleted
                )
                | (
                    ExtensionPointKind::Webhook,
                    InvocationOutcome::WebhookAccepted
                )
                | (
                    ExtensionPointKind::AdminWidget,
                    InvocationOutcome::AdminWidget
                )
                | (
                    ExtensionPointKind::RenderHook,
                    InvocationOutcome::RenderHook
                )
        );

        if !valid {
            return Err(WasmModelError::InvalidOutcomeForPoint {
                handler_id: self.plan.handler_id.to_string(),
                point: self.plan.point,
                outcome: outcome.label(),
            });
        }

        Ok(ExecutionReceipt {
            extension_id: self.plan.extension_id,
            handler_id: self.plan.handler_id,
            point: self.plan.point,
            runtime,
            usage: self.usage,
            outcome,
        })
    }
}
