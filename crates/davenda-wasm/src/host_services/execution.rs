use std::sync::{Arc, Mutex};

use crate::grants::{MetadataGrant, StorageClassGrant};
use crate::host_api::{
    AuthServiceRequest, CacheIntentServiceRequest, DataServiceRequest, HostServiceCall,
    HostServiceRequest, RenderServiceRequest, StorageServiceRequest,
};
use crate::invocation::InvocationContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostServiceExecution {
    pub call: HostServiceCall,
    pub result: HostServiceResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostServiceResult {
    Auth(AuthServiceExecution),
    Data(DataServiceExecution),
    Storage(StorageServiceExecution),
    Render(RenderServiceExecution),
    CacheIntent(CacheIntentExecution),
    Network(NetworkExecution),
    Secret(SecretExecution),
    Job(JobExecution),
    Metadata(MetadataExecution),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthServiceExecution {
    pub request: AuthServiceRequest,
    pub allowed: bool,
    pub checks_seen: u32,
    pub principal_id: Option<String>,
    pub details: AuthServiceDetails,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthServiceDetails {
    Check {
        capability: String,
        object: String,
        decision: bool,
    },
    List {
        capability: String,
        namespace: String,
        object_ids: Vec<String>,
    },
    Lookup {
        capability: String,
        object: String,
        relation: String,
        subject_namespace: String,
        subject_ids: Vec<String>,
    },
    TupleWrite {
        capability: String,
        object: String,
        relation: String,
        subject: String,
        updates: Vec<String>,
        written: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataServiceExecution {
    pub request: DataServiceRequest,
    pub summary: String,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageServiceExecution {
    pub request: StorageServiceRequest,
    pub description: String,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderServiceExecution {
    pub request: RenderServiceRequest,
    pub fragment: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheIntentExecution {
    pub request: CacheIntentServiceRequest,
    pub cache_key: String,
    pub applied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkExecution {
    pub integration: String,
    pub endpoint: String,
    pub status: u16,
    pub response_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretExecution {
    pub secret: String,
    pub source: String,
    pub value_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobExecution {
    pub queue: String,
    pub job_id: String,
    pub enqueued_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataExecution {
    pub kind: MetadataGrant,
    pub recorded: bool,
    pub journal_entries: usize,
}

pub trait HostServiceExecutor: std::fmt::Debug + Send + Sync {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, crate::error::WasmModelError>;
}

#[derive(Debug, Clone, Default)]
pub struct DeniedHostServiceExecutor;

impl HostServiceExecutor for DeniedHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        _context: &InvocationContext,
    ) -> Result<HostServiceExecution, crate::error::WasmModelError> {
        Err(crate::error::WasmModelError::HostServiceUnavailable {
            handler_id: "unknown".to_string(),
            domain: call.domain(),
            reason: "no host service executor configured for this session".to_string(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct SyntheticHostServiceExecutor {
    state: Arc<Mutex<SyntheticHostServiceState>>,
}

impl HostServiceExecutor for SyntheticHostServiceExecutor {
    fn execute(
        &self,
        call: &HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, crate::error::WasmModelError> {
        let mut state = self
            .state
            .lock()
            .expect("synthetic host service state poisoned");
        Ok(state.execute(call.clone(), context))
    }
}

impl Default for SyntheticHostServiceExecutor {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(SyntheticHostServiceState::default())),
        }
    }
}

#[derive(Debug, Default)]
struct SyntheticHostServiceState {
    auth_checks: u32,
    data_sequences: u64,
    storage_bytes_by_class: std::collections::BTreeMap<StorageClassGrant, u64>,
    render_fragments: std::collections::BTreeMap<String, String>,
    cache_hints: Vec<String>,
    metadata_writes: usize,
    job_sequences: u64,
}

impl SyntheticHostServiceState {
    fn execute(
        &mut self,
        call: HostServiceCall,
        context: &InvocationContext,
    ) -> HostServiceExecution {
        let result = match &call.request {
            HostServiceRequest::Auth(request) => {
                self.auth_checks = self.auth_checks.saturating_add(1);
                let principal_id = context.principal.id.clone();
                let details = synthetic_auth_details(request, context, self.auth_checks);
                HostServiceResult::Auth(AuthServiceExecution {
                    request: request.clone(),
                    allowed: true,
                    checks_seen: self.auth_checks,
                    principal_id,
                    details,
                })
            }
            HostServiceRequest::Data(request) => {
                self.data_sequences = self.data_sequences.saturating_add(1);
                HostServiceResult::Data(DataServiceExecution {
                    request: request.clone(),
                    summary: data_summary_for_request(request, context, self.data_sequences),
                    sequence: self.data_sequences,
                })
            }
            HostServiceRequest::Storage(request) => {
                let total_bytes =
                    storage_total_for_request(request, &mut self.storage_bytes_by_class, context);
                HostServiceResult::Storage(StorageServiceExecution {
                    request: request.clone(),
                    description: storage_description_for_request(request, context, total_bytes),
                    total_bytes,
                })
            }
            HostServiceRequest::Render(request) => {
                let fragment = render_fragment_for_request(request, context);
                self.render_fragments
                    .insert(render_fragment_key(request, context), fragment.clone());
                HostServiceResult::Render(RenderServiceExecution {
                    request: request.clone(),
                    fragment,
                })
            }
            HostServiceRequest::CacheIntent(request) => {
                let cache_key = cache_key_for_request(request, context, self.cache_hints.len());
                self.cache_hints.push(cache_key.clone());
                HostServiceResult::CacheIntent(CacheIntentExecution {
                    request: request.clone(),
                    cache_key,
                    applied: true,
                })
            }
            HostServiceRequest::OutboundHttp {
                integration,
                response_bytes,
            } => HostServiceResult::Network(NetworkExecution {
                integration: integration.clone(),
                endpoint: format!("synthetic://{integration}"),
                status: 200,
                response_bytes: *response_bytes,
            }),
            HostServiceRequest::SecretRead { secret } => {
                HostServiceResult::Secret(SecretExecution {
                    secret: secret.clone(),
                    source: "synthetic".to_string(),
                    value_bytes: secret.len(),
                })
            }
            HostServiceRequest::EnqueueJob { queue } => {
                self.job_sequences = self.job_sequences.saturating_add(1);
                HostServiceResult::Job(JobExecution {
                    queue: queue.clone(),
                    job_id: format!("synthetic:{queue}:{}", self.job_sequences),
                    enqueued_at_unix_seconds: self.job_sequences,
                })
            }
            HostServiceRequest::MetadataWrite { kind } => {
                self.metadata_writes = self.metadata_writes.saturating_add(1);
                HostServiceResult::Metadata(MetadataExecution {
                    kind: *kind,
                    recorded: true,
                    journal_entries: self.metadata_writes,
                })
            }
        };

        HostServiceExecution { call, result }
    }
}

#[derive(Debug, Clone)]
pub struct HostServiceJournal {
    executor: Arc<dyn HostServiceExecutor>,
    executions: Vec<HostServiceExecution>,
}

impl HostServiceJournal {
    pub fn new() -> Self {
        Self::with_executor(Arc::new(DeniedHostServiceExecutor::default()))
    }

    pub fn with_executor(executor: Arc<dyn HostServiceExecutor>) -> Self {
        Self {
            executor,
            executions: Vec::new(),
        }
    }

    pub fn executions(&self) -> &[HostServiceExecution] {
        &self.executions
    }

    pub fn execute(
        &mut self,
        call: HostServiceCall,
        context: &InvocationContext,
    ) -> Result<HostServiceExecution, crate::error::WasmModelError> {
        let execution = self.executor.execute(&call, context)?;
        self.executions.push(execution.clone());
        Ok(execution)
    }
}

impl Default for HostServiceJournal {
    fn default() -> Self {
        Self::new()
    }
}

fn data_summary_for_request(
    request: &DataServiceRequest,
    _context: &InvocationContext,
    sequence: u64,
) -> String {
    match request {
        DataServiceRequest::Read { contract } => contract.summary("read", sequence),
        DataServiceRequest::Write { contract } => contract.summary("write", sequence),
    }
}

fn storage_total_for_request(
    request: &StorageServiceRequest,
    storage_bytes_by_class: &mut std::collections::BTreeMap<StorageClassGrant, u64>,
    context: &InvocationContext,
) -> u64 {
    let (class, bytes) = match request {
        StorageServiceRequest::Read { class } => (*class, 0),
        StorageServiceRequest::Write { class, bytes } => (*class, *bytes),
    };
    let total = storage_bytes_by_class
        .entry(class)
        .and_modify(|current| *current = current.saturating_add(bytes))
        .or_insert(bytes);
    let _ = context;
    *total
}

fn storage_description_for_request(
    request: &StorageServiceRequest,
    context: &InvocationContext,
    total_bytes: u64,
) -> String {
    let app_id = &context.customer_app.app_id;
    match request {
        StorageServiceRequest::Read { class } => {
            format!("read storage class {class} for {app_id}")
        }
        StorageServiceRequest::Write { class, bytes } => format!(
            "write {bytes} bytes to storage class {class} for {app_id} (total {total_bytes} bytes)"
        ),
    }
}

fn render_fragment_for_request(
    request: &RenderServiceRequest,
    context: &InvocationContext,
) -> String {
    let locale = context.customer_app.locale.as_deref().unwrap_or("unknown");
    match request {
        RenderServiceRequest::Fragment { slot } => format!(
            "<div data-davenda-slot=\"{slot}\" data-app=\"{}\" data-locale=\"{}\"></div>",
            context.customer_app.app_id, locale
        ),
    }
}

fn render_fragment_key(request: &RenderServiceRequest, context: &InvocationContext) -> String {
    match request {
        RenderServiceRequest::Fragment { slot } => {
            format!("{slot}:{}", context.customer_app.app_id)
        }
    }
}

fn cache_key_for_request(
    request: &CacheIntentServiceRequest,
    context: &InvocationContext,
    cache_intent_count: usize,
) -> String {
    let _ = request;
    format!(
        "cache-intent:{}:{}:{}",
        context.customer_app.app_id, context.trace.trace_id, cache_intent_count
    )
}

fn synthetic_auth_details(
    request: &AuthServiceRequest,
    context: &InvocationContext,
    sequence: u32,
) -> AuthServiceDetails {
    let principal = context
        .principal
        .id
        .clone()
        .unwrap_or_else(|| "anonymous".to_string());
    let tenant = context
        .customer_app
        .tenant_id
        .clone()
        .unwrap_or_else(|| context.customer_app.app_id.clone());

    match request {
        AuthServiceRequest::Check => AuthServiceDetails::Check {
            capability: "system.config.read".to_string(),
            object: format!("tenant:{tenant}"),
            decision: true,
        },
        AuthServiceRequest::List => AuthServiceDetails::List {
            capability: "cms.page.read".to_string(),
            namespace: "page".to_string(),
            object_ids: vec![format!("synthetic-page-{sequence}")],
        },
        AuthServiceRequest::Lookup => AuthServiceDetails::Lookup {
            capability: "system.module.manage".to_string(),
            object: format!("tenant:{tenant}"),
            relation: "manage".to_string(),
            subject_namespace: "user".to_string(),
            subject_ids: vec![principal],
        },
        AuthServiceRequest::TupleWrite => AuthServiceDetails::TupleWrite {
            capability: "system.config.write".to_string(),
            object: format!("tenant:{tenant}"),
            relation: "manage".to_string(),
            subject: principal,
            updates: vec![format!("tenant:{tenant}#manage")],
            written: 1,
        },
    }
}
