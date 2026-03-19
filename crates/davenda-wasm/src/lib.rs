use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WasmModelError {
    EmptyField {
        field: &'static str,
    },
    InvalidToken {
        field: &'static str,
        value: String,
    },
    InvalidRoute {
        field: &'static str,
        route: String,
    },
    DuplicateHandlerId {
        handler_id: String,
    },
    UnsupportedPageMethod {
        method: HttpMethod,
    },
    UnsupportedGrantForPoint {
        handler_id: String,
        point: ExtensionPointKind,
        grant: HostCapabilityGrant,
    },
    HandlerNotFound {
        handler_id: String,
    },
    DuplicateInstalledHandler {
        handler_id: String,
    },
    GrantNotDeclared {
        handler_id: String,
        grant: HostCapabilityGrant,
    },
    LimitOverrideExceedsDeclared {
        handler_id: String,
        field: &'static str,
    },
    ZeroLimit {
        field: &'static str,
    },
    PrincipalIdRequired {
        kind: PrincipalKind,
    },
    InvocationPointMismatch {
        handler_id: String,
        expected: ExtensionPointKind,
        actual: ExtensionPointKind,
    },
    InvocationTargetMismatch {
        handler_id: String,
        detail: String,
    },
    UnverifiedWebhook {
        handler_id: String,
    },
    ReplayUnsafeWebhook {
        handler_id: String,
    },
    HostGrantDenied {
        handler_id: String,
        grant: HostCapabilityGrant,
    },
    ResourceLimitExceeded {
        handler_id: String,
        field: &'static str,
    },
    InvalidOutcomeForPoint {
        handler_id: String,
        point: ExtensionPointKind,
        outcome: &'static str,
    },
    RuntimeBudgetExceeded {
        handler_id: String,
        max_runtime: Duration,
        actual_runtime: Duration,
    },
}

impl fmt::Display for WasmModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField { field } => write!(f, "`{field}` cannot be empty"),
            Self::InvalidToken { field, value } => {
                write!(f, "`{field}` contains an invalid token `{value}`")
            }
            Self::InvalidRoute { field, route } => {
                write!(f, "`{field}` must start with `/`, got `{route}`")
            }
            Self::DuplicateHandlerId { handler_id } => {
                write!(
                    f,
                    "extension manifest declares duplicate handler `{handler_id}`"
                )
            }
            Self::UnsupportedPageMethod { method } => {
                write!(f, "page handlers do not support `{method}`")
            }
            Self::UnsupportedGrantForPoint {
                handler_id,
                point,
                grant,
            } => write!(
                f,
                "handler `{handler_id}` for `{point}` cannot request host grant `{grant}`"
            ),
            Self::HandlerNotFound { handler_id } => {
                write!(
                    f,
                    "installed handler `{handler_id}` does not exist in the manifest"
                )
            }
            Self::DuplicateInstalledHandler { handler_id } => {
                write!(f, "handler `{handler_id}` is installed more than once")
            }
            Self::GrantNotDeclared { handler_id, grant } => write!(
                f,
                "handler `{handler_id}` was granted `{grant}` without declaring it in the manifest"
            ),
            Self::LimitOverrideExceedsDeclared { handler_id, field } => write!(
                f,
                "handler `{handler_id}` has an installation limit override that is looser for `{field}`"
            ),
            Self::ZeroLimit { field } => write!(f, "`{field}` must be greater than zero"),
            Self::PrincipalIdRequired { kind } => {
                write!(f, "principal kind `{kind}` requires a non-empty id")
            }
            Self::InvocationPointMismatch {
                handler_id,
                expected,
                actual,
            } => write!(
                f,
                "handler `{handler_id}` expects invocation point `{expected}` but received `{actual}`"
            ),
            Self::InvocationTargetMismatch { handler_id, detail } => {
                write!(
                    f,
                    "handler `{handler_id}` cannot handle this invocation: {detail}"
                )
            }
            Self::UnverifiedWebhook { handler_id } => write!(
                f,
                "handler `{handler_id}` cannot run until the host verifies the webhook signature"
            ),
            Self::ReplayUnsafeWebhook { handler_id } => write!(
                f,
                "handler `{handler_id}` cannot run until replay protection has been applied"
            ),
            Self::HostGrantDenied { handler_id, grant } => write!(
                f,
                "handler `{handler_id}` attempted host call `{grant}` without a granted capability"
            ),
            Self::ResourceLimitExceeded { handler_id, field } => write!(
                f,
                "handler `{handler_id}` exceeded its `{field}` resource limit"
            ),
            Self::InvalidOutcomeForPoint {
                handler_id,
                point,
                outcome,
            } => write!(
                f,
                "handler `{handler_id}` for `{point}` returned invalid outcome `{outcome}`"
            ),
            Self::RuntimeBudgetExceeded {
                handler_id,
                max_runtime,
                actual_runtime,
            } => write!(
                f,
                "handler `{handler_id}` exceeded runtime budget {:?} with {:?}",
                max_runtime, actual_runtime
            ),
        }
    }
}

impl Error for WasmModelError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ContractVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionId(String);

impl ExtensionId {
    pub fn new(value: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self(validate_token("extension_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandlerId(String);

impl HandlerId {
    pub fn new(value: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self(validate_token("handler_id", value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HandlerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
}

impl fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get => f.write_str("GET"),
            Self::Head => f.write_str("HEAD"),
            Self::Post => f.write_str("POST"),
            Self::Put => f.write_str("PUT"),
            Self::Patch => f.write_str("PATCH"),
            Self::Delete => f.write_str("DELETE"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtensionPointKind {
    Page,
    Api,
    Job,
    ScheduledJob,
    Webhook,
    AdminWidget,
    RenderHook,
}

impl fmt::Display for ExtensionPointKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page => f.write_str("page"),
            Self::Api => f.write_str("api"),
            Self::Job => f.write_str("job"),
            Self::ScheduledJob => f.write_str("scheduled_job"),
            Self::Webhook => f.write_str("webhook"),
            Self::AdminWidget => f.write_str("admin_widget"),
            Self::RenderHook => f.write_str("render_hook"),
        }
    }
}

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
            schedule: require_non_empty("schedule", schedule.into())?,
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

    fn supports_grant(&self, grant: &HostCapabilityGrant) -> bool {
        match grant {
            HostCapabilityGrant::RenderFragment { .. } => matches!(
                self,
                Self::Page(_) | Self::AdminWidget(_) | Self::RenderHook(_)
            ),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageClassGrant {
    PublicUpload,
    PrivateShared,
    LocalOnlySensitive,
    PublicAsset,
}

impl fmt::Display for StorageClassGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PublicUpload => f.write_str("public_upload"),
            Self::PrivateShared => f.write_str("private_shared"),
            Self::LocalOnlySensitive => f.write_str("local_only_sensitive"),
            Self::PublicAsset => f.write_str("public_asset"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetadataGrant {
    JsonLd,
    SitemapEntry,
    Translation,
    SeoHead,
}

impl fmt::Display for MetadataGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::JsonLd => f.write_str("json_ld"),
            Self::SitemapEntry => f.write_str("sitemap_entry"),
            Self::Translation => f.write_str("translation"),
            Self::SeoHead => f.write_str("seo_head"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HostCapabilityGrant {
    DataRead { resource: String },
    DataWrite { resource: String },
    AuthCheck,
    AuthList,
    AuthLookup,
    AuthTupleWrite,
    StorageRead { class: StorageClassGrant },
    StorageWrite { class: StorageClassGrant },
    RenderFragment { slot: String },
    MetadataWrite { kind: MetadataGrant },
    CacheHintWrite,
    OutboundHttp { integration: String },
    SecretRead { secret: String },
    EnqueueJob { queue: String },
}

impl fmt::Display for HostCapabilityGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DataRead { resource } => write!(f, "data.read:{resource}"),
            Self::DataWrite { resource } => write!(f, "data.write:{resource}"),
            Self::AuthCheck => f.write_str("auth.check"),
            Self::AuthList => f.write_str("auth.list"),
            Self::AuthLookup => f.write_str("auth.lookup"),
            Self::AuthTupleWrite => f.write_str("auth.tuple_write"),
            Self::StorageRead { class } => write!(f, "storage.read:{class}"),
            Self::StorageWrite { class } => write!(f, "storage.write:{class}"),
            Self::RenderFragment { slot } => write!(f, "render.fragment:{slot}"),
            Self::MetadataWrite { kind } => write!(f, "metadata.write:{kind}"),
            Self::CacheHintWrite => f.write_str("cache.hint.write"),
            Self::OutboundHttp { integration } => write!(f, "http.outbound:{integration}"),
            Self::SecretRead { secret } => write!(f, "secret.read:{secret}"),
            Self::EnqueueJob { queue } => write!(f, "job.enqueue:{queue}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostGrantSet {
    grants: BTreeSet<HostCapabilityGrant>,
}

impl HostGrantSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_grants(grants: impl IntoIterator<Item = HostCapabilityGrant>) -> Self {
        let mut set = Self::new();
        for grant in grants {
            set.insert(grant);
        }
        set
    }

    pub fn insert(&mut self, grant: HostCapabilityGrant) {
        self.grants.insert(grant);
    }

    pub fn contains(&self, grant: &HostCapabilityGrant) -> bool {
        self.grants.contains(grant)
    }

    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.grants.iter().all(|grant| other.contains(grant))
    }

    pub fn len(&self) -> usize {
        self.grants.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HostCapabilityGrant> {
        self.grants.iter()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceLimits {
    pub max_runtime: Duration,
    pub max_memory_bytes: u64,
    pub max_outbound_requests: u32,
    pub max_outbound_response_bytes: u64,
    pub max_storage_writes: u32,
    pub max_storage_bytes: u64,
    pub max_concurrency: u16,
}

impl ResourceLimits {
    pub const fn new(
        max_runtime: Duration,
        max_memory_bytes: u64,
        max_outbound_requests: u32,
        max_outbound_response_bytes: u64,
        max_storage_writes: u32,
        max_storage_bytes: u64,
        max_concurrency: u16,
    ) -> Self {
        Self {
            max_runtime,
            max_memory_bytes,
            max_outbound_requests,
            max_outbound_response_bytes,
            max_storage_writes,
            max_storage_bytes,
            max_concurrency,
        }
    }

    pub fn baseline_for(point: ExtensionPointKind) -> Self {
        match point {
            ExtensionPointKind::Page
            | ExtensionPointKind::Api
            | ExtensionPointKind::AdminWidget
            | ExtensionPointKind::RenderHook => Self::new(
                Duration::from_secs(2),
                64 * 1024 * 1024,
                4,
                4 * 1024 * 1024,
                2,
                8 * 1024 * 1024,
                32,
            ),
            ExtensionPointKind::Webhook => Self::new(
                Duration::from_secs(5),
                64 * 1024 * 1024,
                6,
                8 * 1024 * 1024,
                2,
                8 * 1024 * 1024,
                16,
            ),
            ExtensionPointKind::Job | ExtensionPointKind::ScheduledJob => Self::new(
                Duration::from_secs(30),
                128 * 1024 * 1024,
                20,
                16 * 1024 * 1024,
                16,
                64 * 1024 * 1024,
                4,
            ),
        }
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        if self.max_runtime.is_zero() {
            return Err(WasmModelError::ZeroLimit {
                field: "max_runtime",
            });
        }
        if self.max_memory_bytes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_memory_bytes",
            });
        }
        if self.max_outbound_requests == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_outbound_requests",
            });
        }
        if self.max_outbound_response_bytes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_outbound_response_bytes",
            });
        }
        if self.max_storage_writes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_storage_writes",
            });
        }
        if self.max_storage_bytes == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_storage_bytes",
            });
        }
        if self.max_concurrency == 0 {
            return Err(WasmModelError::ZeroLimit {
                field: "max_concurrency",
            });
        }

        Ok(())
    }

    fn ensure_no_looser_than(
        &self,
        declared: &Self,
        handler_id: &HandlerId,
    ) -> Result<(), WasmModelError> {
        let checks = [
            (self.max_runtime <= declared.max_runtime, "max_runtime"),
            (
                self.max_memory_bytes <= declared.max_memory_bytes,
                "max_memory_bytes",
            ),
            (
                self.max_outbound_requests <= declared.max_outbound_requests,
                "max_outbound_requests",
            ),
            (
                self.max_outbound_response_bytes <= declared.max_outbound_response_bytes,
                "max_outbound_response_bytes",
            ),
            (
                self.max_storage_writes <= declared.max_storage_writes,
                "max_storage_writes",
            ),
            (
                self.max_storage_bytes <= declared.max_storage_bytes,
                "max_storage_bytes",
            ),
            (
                self.max_concurrency <= declared.max_concurrency,
                "max_concurrency",
            ),
        ];

        for (passes, field) in checks {
            if !passes {
                return Err(WasmModelError::LimitOverrideExceedsDeclared {
                    handler_id: handler_id.to_string(),
                    field,
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerManifest {
    pub id: HandlerId,
    pub export: String,
    pub point: ExtensionPoint,
    pub requested_grants: HostGrantSet,
    pub limits: Option<ResourceLimits>,
}

impl HandlerManifest {
    pub fn new(
        id: HandlerId,
        export: impl Into<String>,
        point: ExtensionPoint,
        requested_grants: HostGrantSet,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            id,
            export: validate_token("export", export.into())?,
            point,
            requested_grants,
            limits: None,
        })
    }

    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    pub fn effective_limits(&self, manifest_defaults: ResourceLimits) -> ResourceLimits {
        self.limits.unwrap_or(manifest_defaults)
    }

    fn validate(&self, manifest_defaults: ResourceLimits) -> Result<(), WasmModelError> {
        let effective_limits = self.effective_limits(manifest_defaults);
        effective_limits.validate()?;

        for grant in self.requested_grants.iter() {
            if !self.point.supports_grant(grant) {
                return Err(WasmModelError::UnsupportedGrantForPoint {
                    handler_id: self.id.to_string(),
                    point: self.point.kind(),
                    grant: grant.clone(),
                });
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionManifest {
    pub id: ExtensionId,
    pub display_name: String,
    pub version: ContractVersion,
    pub host_api_version: ContractVersion,
    pub default_limits: ResourceLimits,
    pub handlers: Vec<HandlerManifest>,
}

impl ExtensionManifest {
    pub fn new(
        id: ExtensionId,
        display_name: impl Into<String>,
        version: ContractVersion,
        host_api_version: ContractVersion,
        default_limits: ResourceLimits,
        handlers: Vec<HandlerManifest>,
    ) -> Result<Self, WasmModelError> {
        let manifest = Self {
            id,
            display_name: require_non_empty("display_name", display_name.into())?,
            version,
            host_api_version,
            default_limits,
            handlers,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        self.default_limits.validate()?;
        let mut seen = BTreeSet::new();
        for handler in &self.handlers {
            if !seen.insert(handler.id.clone()) {
                return Err(WasmModelError::DuplicateHandlerId {
                    handler_id: handler.id.to_string(),
                });
            }

            handler.validate(self.default_limits)?;
        }
        Ok(())
    }

    pub fn handler(&self, id: &HandlerId) -> Option<&HandlerManifest> {
        self.handlers.iter().find(|handler| &handler.id == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledHandler {
    pub handler_id: HandlerId,
    pub granted_capabilities: HostGrantSet,
    pub effective_limits: ResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerInstallation {
    pub handler_id: HandlerId,
    pub granted_capabilities: HostGrantSet,
    pub limit_override: Option<ResourceLimits>,
}

impl HandlerInstallation {
    pub fn new(handler_id: HandlerId, granted_capabilities: HostGrantSet) -> Self {
        Self {
            handler_id,
            granted_capabilities,
            limit_override: None,
        }
    }

    pub fn with_limit_override(mut self, limits: ResourceLimits) -> Self {
        self.limit_override = Some(limits);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionInstallation {
    pub customer_app_id: String,
    pub handlers: Vec<HandlerInstallation>,
}

impl ExtensionInstallation {
    pub fn new(
        customer_app_id: impl Into<String>,
        handlers: Vec<HandlerInstallation>,
    ) -> Result<Self, WasmModelError> {
        Ok(Self {
            customer_app_id: validate_token("customer_app_id", customer_app_id.into())?,
            handlers,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtension {
    manifest: ExtensionManifest,
    customer_app_id: String,
    handlers: BTreeMap<HandlerId, InstalledHandler>,
}

impl InstalledExtension {
    pub fn install(
        manifest: ExtensionManifest,
        installation: ExtensionInstallation,
    ) -> Result<Self, WasmModelError> {
        manifest.validate()?;

        let mut handlers = BTreeMap::new();
        for configured_handler in installation.handlers {
            let manifest_handler = manifest
                .handler(&configured_handler.handler_id)
                .ok_or_else(|| WasmModelError::HandlerNotFound {
                    handler_id: configured_handler.handler_id.to_string(),
                })?;

            if handlers.contains_key(&configured_handler.handler_id) {
                return Err(WasmModelError::DuplicateInstalledHandler {
                    handler_id: configured_handler.handler_id.to_string(),
                });
            }

            if !configured_handler
                .granted_capabilities
                .is_subset_of(&manifest_handler.requested_grants)
            {
                let offending = configured_handler
                    .granted_capabilities
                    .iter()
                    .find(|grant| !manifest_handler.requested_grants.contains(grant))
                    .expect("subset failure has an offending grant")
                    .clone();

                return Err(WasmModelError::GrantNotDeclared {
                    handler_id: configured_handler.handler_id.to_string(),
                    grant: offending,
                });
            }

            let declared_limits = manifest_handler.effective_limits(manifest.default_limits);
            let effective_limits = configured_handler.limit_override.unwrap_or(declared_limits);
            effective_limits.validate()?;
            effective_limits
                .ensure_no_looser_than(&declared_limits, &configured_handler.handler_id)?;

            handlers.insert(
                configured_handler.handler_id.clone(),
                InstalledHandler {
                    handler_id: configured_handler.handler_id,
                    granted_capabilities: configured_handler.granted_capabilities,
                    effective_limits,
                },
            );
        }

        Ok(Self {
            manifest,
            customer_app_id: installation.customer_app_id,
            handlers,
        })
    }

    pub fn manifest(&self) -> &ExtensionManifest {
        &self.manifest
    }

    pub fn customer_app_id(&self) -> &str {
        &self.customer_app_id
    }

    pub fn prepare_invocation(
        &self,
        handler_id: &HandlerId,
        context: InvocationContext,
    ) -> Result<InvocationPlan, WasmModelError> {
        context.validate()?;

        let manifest_handler =
            self.manifest
                .handler(handler_id)
                .ok_or_else(|| WasmModelError::HandlerNotFound {
                    handler_id: handler_id.to_string(),
                })?;
        let installed_handler =
            self.handlers
                .get(handler_id)
                .ok_or_else(|| WasmModelError::HandlerNotFound {
                    handler_id: handler_id.to_string(),
                })?;

        let actual_point = context.input.kind();
        let expected_point = manifest_handler.point.kind();
        if actual_point != expected_point {
            return Err(WasmModelError::InvocationPointMismatch {
                handler_id: handler_id.to_string(),
                expected: expected_point,
                actual: actual_point,
            });
        }

        validate_invocation_target(handler_id, &manifest_handler.point, &context.input)?;

        Ok(InvocationPlan {
            extension_id: self.manifest.id.clone(),
            handler_id: handler_id.clone(),
            point: manifest_handler.point.kind(),
            customer_app_id: self.customer_app_id.clone(),
            granted_capabilities: installed_handler.granted_capabilities.clone(),
            limits: installed_handler.effective_limits,
            context,
        })
    }
}

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
            app_id: validate_token("app_id", app_id.into())?,
            tenant_id: None,
            site_id: None,
            brand_id: None,
            locale: None,
        })
    }

    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.tenant_id = Some(validate_token("tenant_id", tenant_id.into())?);
        Ok(self)
    }

    pub fn with_site_id(mut self, site_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.site_id = Some(validate_token("site_id", site_id.into())?);
        Ok(self)
    }

    pub fn with_brand_id(mut self, brand_id: impl Into<String>) -> Result<Self, WasmModelError> {
        self.brand_id = Some(validate_token("brand_id", brand_id.into())?);
        Ok(self)
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Result<Self, WasmModelError> {
        self.locale = Some(validate_token("locale", locale.into())?);
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
            id: Some(validate_token("principal_id", id.into())?),
        })
    }

    pub fn service_account(id: impl Into<String>) -> Result<Self, WasmModelError> {
        Ok(Self {
            kind: PrincipalKind::ServiceAccount,
            id: Some(validate_token("principal_id", id.into())?),
        })
    }

    fn validate(&self) -> Result<(), WasmModelError> {
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
            trace_id: validate_token("trace_id", trace_id.into())?,
            parent_span_id: None,
            request_id: None,
        })
    }

    pub fn with_parent_span_id(
        mut self,
        parent_span_id: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.parent_span_id = Some(validate_token("parent_span_id", parent_span_id.into())?);
        Ok(self)
    }

    pub fn with_request_id(
        mut self,
        request_id: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        self.request_id = Some(validate_token("request_id", request_id.into())?);
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
            route: validate_route("page_invocation_route", route.into())?,
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
            route: validate_route("api_invocation_route", route.into())?,
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
            job_name: validate_token("job_invocation_name", job_name.into())?,
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
            job_name: validate_token("scheduled_job_invocation_name", job_name.into())?,
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
            source: validate_token("webhook_invocation_source", source.into())?,
            event: validate_token("webhook_invocation_event", event.into())?,
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
            slot: validate_token("admin_widget_invocation_slot", slot.into())?,
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
            slot: validate_token("render_hook_invocation_slot", slot.into())?,
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
    pub extension_config: BTreeMap<String, String>,
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
        value: impl Into<String>,
    ) -> Result<Self, WasmModelError> {
        let key = validate_token("extension_config_key", key.into())?;
        let value = require_non_empty("extension_config_value", value.into())?;
        self.extension_config.insert(key, value);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), WasmModelError> {
        self.principal.validate()?;
        for (key, value) in &self.extension_config {
            validate_token("extension_config_key", key.clone())?;
            require_non_empty("extension_config_value", value.clone())?;
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
    DataRead { resource: String },
    DataWrite { resource: String },
    AuthCheck,
    AuthList,
    AuthLookup,
    AuthTupleWrite,
    StorageRead { class: StorageClassGrant },
    StorageWrite { class: StorageClassGrant, bytes: u64 },
    RenderFragment { slot: String },
    MetadataWrite { kind: MetadataGrant },
    CacheHintWrite,
    OutboundHttp {
        integration: String,
        response_bytes: u64,
    },
    SecretRead { secret: String },
    EnqueueJob { queue: String },
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
            Self::RenderFragment { slot } => HostCapabilityGrant::RenderFragment { slot: slot.clone() },
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
                if self.usage.outbound_response_bytes
                    > self.plan.limits.max_outbound_response_bytes
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
                | (ExtensionPointKind::Webhook, InvocationOutcome::WebhookAccepted)
                | (ExtensionPointKind::AdminWidget, InvocationOutcome::AdminWidget)
                | (ExtensionPointKind::RenderHook, InvocationOutcome::RenderHook)
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

fn validate_invocation_target(
    handler_id: &HandlerId,
    point: &ExtensionPoint,
    input: &InvocationInput,
) -> Result<(), WasmModelError> {
    match (point, input) {
        (ExtensionPoint::Page(page), InvocationInput::Page(invocation)) => {
            if page.route != invocation.route {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "page route `{}` does not match registered route `{}`",
                        invocation.route, page.route
                    ),
                });
            }
            if !page.methods.contains(&invocation.method) {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "page method `{}` is not enabled for `{}`",
                        invocation.method, page.route
                    ),
                });
            }
        }
        (ExtensionPoint::Api(api), InvocationInput::Api(invocation)) => {
            if api.route != invocation.route {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "api route `{}` does not match registered route `{}`",
                        invocation.route, api.route
                    ),
                });
            }
            if !api.methods.contains(&invocation.method) {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "api method `{}` is not enabled for `{}`",
                        invocation.method, api.route
                    ),
                });
            }
        }
        (ExtensionPoint::Job(job), InvocationInput::Job(invocation)) => {
            if job.job_name != invocation.job_name {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "job `{}` does not match registered job `{}`",
                        invocation.job_name, job.job_name
                    ),
                });
            }
        }
        (ExtensionPoint::ScheduledJob(job), InvocationInput::ScheduledJob(invocation)) => {
            if job.job_name != invocation.job_name {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "scheduled job `{}` does not match registered job `{}`",
                        invocation.job_name, job.job_name
                    ),
                });
            }
        }
        (ExtensionPoint::Webhook(webhook), InvocationInput::Webhook(invocation)) => {
            if !invocation.verified {
                return Err(WasmModelError::UnverifiedWebhook {
                    handler_id: handler_id.to_string(),
                });
            }
            if !invocation.replay_protected {
                return Err(WasmModelError::ReplayUnsafeWebhook {
                    handler_id: handler_id.to_string(),
                });
            }
            if webhook.source != invocation.source || webhook.event != invocation.event {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "webhook `{}/{}`
 does not match registered `{}/{}`",
                        invocation.source, invocation.event, webhook.source, webhook.event
                    )
                    .replace('\n', ""),
                });
            }
        }
        (ExtensionPoint::AdminWidget(widget), InvocationInput::AdminWidget(invocation)) => {
            if widget.slot != invocation.slot {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "admin slot `{}` does not match registered slot `{}`",
                        invocation.slot, widget.slot
                    ),
                });
            }
        }
        (ExtensionPoint::RenderHook(hook), InvocationInput::RenderHook(invocation)) => {
            if hook.slot != invocation.slot {
                return Err(WasmModelError::InvocationTargetMismatch {
                    handler_id: handler_id.to_string(),
                    detail: format!(
                        "render slot `{}` does not match registered slot `{}`",
                        invocation.slot, hook.slot
                    ),
                });
            }
        }
        _ => {
            return Err(WasmModelError::InvocationPointMismatch {
                handler_id: handler_id.to_string(),
                expected: point.kind(),
                actual: input.kind(),
            });
        }
    }

    Ok(())
}

fn require_non_empty(field: &'static str, value: String) -> Result<String, WasmModelError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(WasmModelError::EmptyField { field })
    } else {
        Ok(trimmed.to_string())
    }
}

fn validate_token(field: &'static str, value: String) -> Result<String, WasmModelError> {
    let trimmed = require_non_empty(field, value)?;
    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '/'))
    {
        Ok(trimmed)
    } else {
        Err(WasmModelError::InvalidToken {
            field,
            value: trimmed,
        })
    }
}

fn validate_route(field: &'static str, route: String) -> Result<String, WasmModelError> {
    let route = require_non_empty(field, route)?;
    if route.starts_with('/') {
        Ok(route)
    } else {
        Err(WasmModelError::InvalidRoute { field, route })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_limits() -> ResourceLimits {
        ResourceLimits::baseline_for(ExtensionPointKind::Page)
    }

    fn page_manifest() -> ExtensionManifest {
        let page_handler = HandlerManifest::new(
            HandlerId::new("waitlist-page").unwrap(),
            "exports.page_waitlist",
            ExtensionPoint::Page(
                PageExtensionPoint::new("/events/waitlist", [HttpMethod::Get, HttpMethod::Post])
                    .unwrap(),
            ),
            HostGrantSet::from_grants([
                HostCapabilityGrant::DataRead {
                    resource: "events.waitlist".to_string(),
                },
                HostCapabilityGrant::AuthCheck,
                HostCapabilityGrant::RenderFragment {
                    slot: "events.waitlist.panel".to_string(),
                },
                HostCapabilityGrant::CacheHintWrite,
            ]),
        )
        .unwrap();

        ExtensionManifest::new(
            ExtensionId::new("events.waitlist").unwrap(),
            "Events Waitlist Tools",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            default_limits(),
            vec![page_handler],
        )
        .unwrap()
    }

    #[test]
    fn manifest_rejects_visual_render_grants_on_job_handlers() {
        let handler = HandlerManifest::new(
            HandlerId::new("reconcile-job").unwrap(),
            "exports.reconcile",
            ExtensionPoint::Job(JobExtensionPoint::new("reconcile", "default").unwrap()),
            HostGrantSet::from_grants([HostCapabilityGrant::RenderFragment {
                slot: "admin.dashboard".to_string(),
            }]),
        )
        .unwrap();

        let error = ExtensionManifest::new(
            ExtensionId::new("jobs.reconcile").unwrap(),
            "Reconcile Jobs",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Job),
            vec![handler],
        )
        .unwrap_err();

        assert_eq!(
            error,
            WasmModelError::UnsupportedGrantForPoint {
                handler_id: "reconcile-job".to_string(),
                point: ExtensionPointKind::Job,
                grant: HostCapabilityGrant::RenderFragment {
                    slot: "admin.dashboard".to_string(),
                },
            }
        );
    }

    #[test]
    fn installation_rejects_grants_that_were_not_declared() {
        let manifest = page_manifest();
        let installation = ExtensionInstallation::new(
            "customer-app",
            vec![HandlerInstallation::new(
                HandlerId::new("waitlist-page").unwrap(),
                HostGrantSet::from_grants([
                    HostCapabilityGrant::AuthCheck,
                    HostCapabilityGrant::SecretRead {
                        secret: "undocumented".to_string(),
                    },
                ]),
            )],
        )
        .unwrap();

        let error = InstalledExtension::install(manifest, installation).unwrap_err();
        assert_eq!(
            error,
            WasmModelError::GrantNotDeclared {
                handler_id: "waitlist-page".to_string(),
                grant: HostCapabilityGrant::SecretRead {
                    secret: "undocumented".to_string(),
                },
            }
        );
    }

    #[test]
    fn installation_limit_overrides_must_only_tighten_declared_limits() {
        let manifest = page_manifest();
        let installation = ExtensionInstallation::new(
            "customer-app",
            vec![
                HandlerInstallation::new(
                    HandlerId::new("waitlist-page").unwrap(),
                    HostGrantSet::from_grants([HostCapabilityGrant::AuthCheck]),
                )
                .with_limit_override(ResourceLimits::new(
                    Duration::from_secs(5),
                    default_limits().max_memory_bytes,
                    default_limits().max_outbound_requests,
                    default_limits().max_outbound_response_bytes,
                    default_limits().max_storage_writes,
                    default_limits().max_storage_bytes,
                    default_limits().max_concurrency,
                )),
            ],
        )
        .unwrap();

        let error = InstalledExtension::install(manifest, installation).unwrap_err();
        assert_eq!(
            error,
            WasmModelError::LimitOverrideExceedsDeclared {
                handler_id: "waitlist-page".to_string(),
                field: "max_runtime",
            }
        );
    }

    #[test]
    fn installed_extension_prepares_invocation_with_granted_capabilities_and_limits() {
        let manifest = page_manifest();
        let installed = InstalledExtension::install(
            manifest,
            ExtensionInstallation::new(
                "customer-app",
                vec![
                    HandlerInstallation::new(
                        HandlerId::new("waitlist-page").unwrap(),
                        HostGrantSet::from_grants([
                            HostCapabilityGrant::AuthCheck,
                            HostCapabilityGrant::DataRead {
                                resource: "events.waitlist".to_string(),
                            },
                        ]),
                    )
                    .with_limit_override(ResourceLimits::new(
                        Duration::from_secs(1),
                        32 * 1024 * 1024,
                        2,
                        2 * 1024 * 1024,
                        1,
                        2 * 1024 * 1024,
                        8,
                    )),
                ],
            )
            .unwrap(),
        )
        .unwrap();

        let plan = installed
            .prepare_invocation(
                &HandlerId::new("waitlist-page").unwrap(),
                InvocationContext::new(
                    CustomerAppContext::new("customer-app")
                        .unwrap()
                        .with_site_id("main-site")
                        .unwrap()
                        .with_locale("en-GB")
                        .unwrap(),
                    PrincipalRef::user("user-42").unwrap(),
                    TraceContext::new("trace-123")
                        .unwrap()
                        .with_request_id("req-99")
                        .unwrap(),
                    InvocationInput::Page(
                        PageInvocation::new("/events/waitlist", HttpMethod::Post).unwrap(),
                    ),
                ),
            )
            .unwrap();

        assert_eq!(plan.point, ExtensionPointKind::Page);
        assert_eq!(plan.customer_app_id, "customer-app");
        assert_eq!(plan.granted_capabilities.len(), 2);
        assert_eq!(plan.limits.max_runtime, Duration::from_secs(1));
        assert_eq!(plan.context.customer_app.locale.as_deref(), Some("en-GB"));
    }

    #[test]
    fn webhook_invocations_require_host_verification_and_replay_protection() {
        let manifest = ExtensionManifest::new(
            ExtensionId::new("tickets.ingest").unwrap(),
            "Ticketing Ingest",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Webhook),
            vec![
                HandlerManifest::new(
                    HandlerId::new("webhook-ingest").unwrap(),
                    "exports.handle_webhook",
                    ExtensionPoint::Webhook(
                        WebhookExtensionPoint::new("ticketing", "reservation.updated").unwrap(),
                    ),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::DataWrite {
                            resource: "events.reservation".to_string(),
                        },
                        HostCapabilityGrant::EnqueueJob {
                            queue: "follow-up".to_string(),
                        },
                    ]),
                )
                .unwrap(),
            ],
        )
        .unwrap();

        let installed = InstalledExtension::install(
            manifest,
            ExtensionInstallation::new(
                "customer-app",
                vec![HandlerInstallation::new(
                    HandlerId::new("webhook-ingest").unwrap(),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::DataWrite {
                            resource: "events.reservation".to_string(),
                        },
                        HostCapabilityGrant::EnqueueJob {
                            queue: "follow-up".to_string(),
                        },
                    ]),
                )],
            )
            .unwrap(),
        )
        .unwrap();

        let error = installed
            .prepare_invocation(
                &HandlerId::new("webhook-ingest").unwrap(),
                InvocationContext::new(
                    CustomerAppContext::new("customer-app").unwrap(),
                    PrincipalRef::service_account("svc-ingest").unwrap(),
                    TraceContext::new("trace-webhook").unwrap(),
                    InvocationInput::Webhook(
                        WebhookInvocation::new("ticketing", "reservation.updated", false, true)
                            .unwrap(),
                    ),
                ),
            )
            .unwrap_err();

        assert_eq!(
            error,
            WasmModelError::UnverifiedWebhook {
                handler_id: "webhook-ingest".to_string(),
            }
        );
    }

    #[test]
    fn manifest_rejects_duplicate_handler_ids() {
        let handler = HandlerManifest::new(
            HandlerId::new("shared-id").unwrap(),
            "exports.one",
            ExtensionPoint::RenderHook(RenderHookExtensionPoint::new("slot.one").unwrap()),
            HostGrantSet::from_grants([
                HostCapabilityGrant::RenderFragment {
                    slot: "slot.one".to_string(),
                },
                HostCapabilityGrant::MetadataWrite {
                    kind: MetadataGrant::JsonLd,
                },
            ]),
        )
        .unwrap();

        let error = ExtensionManifest::new(
            ExtensionId::new("duplicate.handlers").unwrap(),
            "Duplicate Handler Test",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::RenderHook),
            vec![handler.clone(), handler],
        )
        .unwrap_err();

        assert_eq!(
            error,
            WasmModelError::DuplicateHandlerId {
                handler_id: "shared-id".to_string(),
            }
        );
    }

    #[test]
    fn execution_session_enforces_host_grants_and_resource_limits() {
        let manifest = ExtensionManifest::new(
            ExtensionId::new("events.waitlist.exec").unwrap(),
            "Events Waitlist Execution",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            default_limits(),
            vec![
                HandlerManifest::new(
                    HandlerId::new("waitlist-page").unwrap(),
                    "exports.page_waitlist",
                    ExtensionPoint::Page(
                        PageExtensionPoint::new(
                            "/events/waitlist",
                            [HttpMethod::Get, HttpMethod::Post],
                        )
                        .unwrap(),
                    ),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::AuthCheck,
                        HostCapabilityGrant::OutboundHttp {
                            integration: "crm".to_string(),
                        },
                        HostCapabilityGrant::StorageWrite {
                            class: StorageClassGrant::PrivateShared,
                        },
                    ]),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let plan = InstalledExtension::install(
            manifest,
            ExtensionInstallation::new(
                "customer-app",
                vec![HandlerInstallation::new(
                    HandlerId::new("waitlist-page").unwrap(),
                    HostGrantSet::from_grants([
                        HostCapabilityGrant::AuthCheck,
                        HostCapabilityGrant::OutboundHttp {
                            integration: "crm".to_string(),
                        },
                        HostCapabilityGrant::StorageWrite {
                            class: StorageClassGrant::PrivateShared,
                        },
                    ]),
                )],
            )
            .unwrap(),
        )
        .unwrap()
        .prepare_invocation(
            &HandlerId::new("waitlist-page").unwrap(),
            InvocationContext::new(
                CustomerAppContext::new("customer-app").unwrap(),
                PrincipalRef::user("user-42").unwrap(),
                TraceContext::new("trace-1").unwrap(),
                InvocationInput::Page(
                    PageInvocation::new("/events/waitlist", HttpMethod::Get).unwrap(),
                ),
            ),
        )
        .unwrap();

        let mut session = plan.begin_execution();
        session.record_host_call(HostCall::AuthCheck).unwrap();
        session
            .record_host_call(HostCall::OutboundHttp {
                integration: "crm".to_string(),
                response_bytes: 512,
            })
            .unwrap();
        session
            .record_host_call(HostCall::StorageWrite {
                class: StorageClassGrant::PrivateShared,
                bytes: 1_024,
            })
            .unwrap();
        let denied = session
            .record_host_call(HostCall::SecretRead {
                secret: "tls-account".to_string(),
            })
            .unwrap_err();
        assert_eq!(
            denied,
            WasmModelError::HostGrantDenied {
                handler_id: "waitlist-page".to_string(),
                grant: HostCapabilityGrant::SecretRead {
                    secret: "tls-account".to_string(),
                },
            }
        );
    }

    #[test]
    fn execution_session_rejects_invalid_outcomes_and_runtime_overruns() {
        let manifest = ExtensionManifest::new(
            ExtensionId::new("jobs.reconcile").unwrap(),
            "Reconcile Jobs",
            ContractVersion::new(1, 0, 0),
            ContractVersion::new(1, 0, 0),
            ResourceLimits::baseline_for(ExtensionPointKind::Job),
            vec![
                HandlerManifest::new(
                    HandlerId::new("reconcile-job").unwrap(),
                    "exports.reconcile",
                    ExtensionPoint::Job(JobExtensionPoint::new("reconcile", "jobs.work").unwrap()),
                    HostGrantSet::from_grants([HostCapabilityGrant::DataWrite {
                        resource: "billing.invoice".to_string(),
                    }]),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let plan = InstalledExtension::install(
            manifest,
            ExtensionInstallation::new(
                "customer-app",
                vec![HandlerInstallation::new(
                    HandlerId::new("reconcile-job").unwrap(),
                    HostGrantSet::from_grants([HostCapabilityGrant::DataWrite {
                        resource: "billing.invoice".to_string(),
                    }]),
                )],
            )
            .unwrap(),
        )
        .unwrap()
        .prepare_invocation(
            &HandlerId::new("reconcile-job").unwrap(),
            InvocationContext::new(
                CustomerAppContext::new("customer-app").unwrap(),
                PrincipalRef::service_account("svc-jobs").unwrap(),
                TraceContext::new("trace-job").unwrap(),
                InvocationInput::Job(JobInvocation::new("reconcile", 1).unwrap()),
            ),
        )
        .unwrap();

        let invalid = plan
            .clone()
            .begin_execution()
            .finish(Duration::from_secs(1), InvocationOutcome::ApiJson)
            .unwrap_err();
        assert_eq!(
            invalid,
            WasmModelError::InvalidOutcomeForPoint {
                handler_id: "reconcile-job".to_string(),
                point: ExtensionPointKind::Job,
                outcome: "api_json",
            }
        );

        let over_budget = plan
            .begin_execution()
            .finish(Duration::from_secs(31), InvocationOutcome::JobCompleted)
            .unwrap_err();
        assert!(matches!(
            over_budget,
            WasmModelError::RuntimeBudgetExceeded { .. }
        ));
    }

    #[test]
    fn execution_session_tracks_peak_concurrency() {
        let manifest = page_manifest();
        let plan = InstalledExtension::install(
            manifest,
            ExtensionInstallation::new(
                "customer-app",
                vec![
                    HandlerInstallation::new(
                        HandlerId::new("waitlist-page").unwrap(),
                        HostGrantSet::from_grants([
                            HostCapabilityGrant::AuthCheck,
                            HostCapabilityGrant::DataRead {
                                resource: "events.waitlist".to_string(),
                            },
                        ]),
                    )
                    .with_limit_override(ResourceLimits::new(
                        Duration::from_secs(2),
                        64 * 1024 * 1024,
                        4,
                        4 * 1024 * 1024,
                        2,
                        8 * 1024 * 1024,
                        2,
                    )),
                ],
            )
            .unwrap(),
        )
        .unwrap()
        .prepare_invocation(
            &HandlerId::new("waitlist-page").unwrap(),
            InvocationContext::new(
                CustomerAppContext::new("customer-app").unwrap(),
                PrincipalRef::user("user-7").unwrap(),
                TraceContext::new("trace-2").unwrap(),
                InvocationInput::Page(
                    PageInvocation::new("/events/waitlist", HttpMethod::Post).unwrap(),
                ),
            ),
        )
        .unwrap();

        let mut session = plan.begin_execution();
        session.reserve_concurrency(1).unwrap();
        session.reserve_concurrency(1).unwrap();
        let err = session.reserve_concurrency(1).unwrap_err();
        assert_eq!(
            err,
            WasmModelError::ResourceLimitExceeded {
                handler_id: "waitlist-page".to_string(),
                field: "max_concurrency",
            }
        );
    }
}
