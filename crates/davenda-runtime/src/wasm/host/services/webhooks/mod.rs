use std::path::{Path, PathBuf};

use super::super::*;

mod local;
mod shared;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookObservationStatus {
    Accepted,
    VerificationFailed,
    ReplayRejected,
    ExecutionFailed,
}

impl WebhookObservationStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::VerificationFailed => "verification_failed",
            Self::ReplayRejected => "replay_rejected",
            Self::ExecutionFailed => "execution_failed",
        }
    }

    pub(crate) fn from_db_value(value: &str) -> Result<Self, String> {
        match value {
            "accepted" => Ok(Self::Accepted),
            "verification_failed" => Ok(Self::VerificationFailed),
            "replay_rejected" => Ok(Self::ReplayRejected),
            "execution_failed" => Ok(Self::ExecutionFailed),
            other => Err(format!("unknown webhook observation status `{other}`")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WebhookObservationStatusCounts {
    pub accepted: usize,
    pub verification_failed: usize,
    pub replay_rejected: usize,
    pub execution_failed: usize,
}

impl WebhookObservationStatusCounts {
    fn increment(&mut self, status: WebhookObservationStatus, count: usize) {
        match status {
            WebhookObservationStatus::Accepted => self.accepted += count,
            WebhookObservationStatus::VerificationFailed => self.verification_failed += count,
            WebhookObservationStatus::ReplayRejected => self.replay_rejected += count,
            WebhookObservationStatus::ExecutionFailed => self.execution_failed += count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookObservationBackendKind {
    LocalSqlite,
    SharedPostgres,
}

impl WebhookObservationBackendKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::LocalSqlite => "local-sqlite",
            Self::SharedPostgres => "shared-postgres",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookObservationEvent {
    pub id: i64,
    pub recorded_at_unix_seconds: i64,
    pub app_id: String,
    pub source: String,
    pub event: String,
    pub status: WebhookObservationStatus,
    pub trace_id: String,
    pub principal_kind: String,
    pub principal_id: Option<String>,
    pub detail: Option<String>,
}

impl WebhookObservationEvent {
    fn from_context(
        source: &str,
        event: &str,
        status: WebhookObservationStatus,
        context: &InvocationContext,
        detail: Option<String>,
    ) -> Self {
        Self {
            id: 0,
            recorded_at_unix_seconds: unix_seconds_now(),
            app_id: context.customer_app.app_id.clone(),
            source: source.to_string(),
            event: event.to_string(),
            status,
            trace_id: context.trace.trace_id.clone(),
            principal_kind: context.principal.kind.to_string(),
            principal_id: context.principal.id.clone(),
            detail,
        }
    }

    fn from_request(
        app_id: &str,
        source: &str,
        event: &str,
        status: WebhookObservationStatus,
        request_id: &str,
        principal_kind: &str,
        principal_id: Option<&str>,
        detail: Option<String>,
    ) -> Self {
        Self {
            id: 0,
            recorded_at_unix_seconds: unix_seconds_now(),
            app_id: app_id.to_string(),
            source: source.to_string(),
            event: event.to_string(),
            status,
            trace_id: request_id.to_string(),
            principal_kind: principal_kind.to_string(),
            principal_id: principal_id.map(str::to_string),
            detail,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookObservationSnapshot {
    pub backend: WebhookObservationBackendKind,
    pub location: String,
    pub path: Option<PathBuf>,
    pub entry_count: usize,
    pub status_counts: WebhookObservationStatusCounts,
    pub recent_events: Vec<WebhookObservationEvent>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeWebhookObservationBackend {
    backend: WebhookObservationBackend,
}

impl RuntimeWebhookObservationBackend {
    pub(super) fn open(plan: &RuntimePlan) -> Self {
        let backend = match plan.metadata_audit_backend_selection() {
            crate::plan::MetadataAuditBackendSelection::SharedPostgres { runtime } => {
                WebhookObservationBackend::shared(shared::SharedWebhookObservationStore::open(
                    runtime,
                ))
            }
            crate::plan::MetadataAuditBackendSelection::LocalSqlite { root, namespace } => {
                WebhookObservationBackend::local(local::LocalWebhookObservationStore::open(
                    root, namespace,
                ))
            }
        };
        Self { backend }
    }

    #[cfg(test)]
    pub(super) fn with_local_root(root: impl Into<PathBuf>, namespace: impl Into<String>) -> Self {
        Self {
            backend: WebhookObservationBackend::local(local::LocalWebhookObservationStore::open(
                root.into(),
                namespace.into(),
            )),
        }
    }

    pub(super) fn record(
        &self,
        source: &str,
        event: &str,
        status: WebhookObservationStatus,
        context: &InvocationContext,
        detail: Option<String>,
    ) -> Result<(), String> {
        self.backend.insert(&WebhookObservationEvent::from_context(
            source, event, status, context, detail,
        ))
    }

    pub(super) fn record_request(
        &self,
        app_id: &str,
        source: &str,
        event: &str,
        status: WebhookObservationStatus,
        request_id: &str,
        principal_kind: &str,
        principal_id: Option<&str>,
        detail: Option<String>,
    ) -> Result<(), String> {
        self.backend.insert(&WebhookObservationEvent::from_request(
            app_id,
            source,
            event,
            status,
            request_id,
            principal_kind,
            principal_id,
            detail,
        ))
    }

    pub(super) fn claim_delivery(
        &self,
        app_id: &str,
        route_name: &str,
        source: &str,
        delivery_id: &str,
        request_id: &str,
        recorded_at_unix_seconds: i64,
    ) -> Result<bool, String> {
        self.backend.claim_delivery(
            app_id,
            route_name,
            source,
            delivery_id,
            request_id,
            recorded_at_unix_seconds,
        )
    }

    pub(super) fn snapshot(&self, limit: usize) -> Result<WebhookObservationSnapshot, String> {
        Ok(WebhookObservationSnapshot {
            backend: self.backend.kind(),
            location: self.backend.location_label(),
            path: self.backend.path().map(Path::to_path_buf),
            entry_count: self.backend.entry_count()?,
            status_counts: self.backend.status_counts()?,
            recent_events: self.backend.recent(limit)?,
        })
    }
}

#[derive(Debug, Clone)]
enum WebhookObservationBackend {
    Local(local::LocalWebhookObservationStore),
    Shared(shared::SharedWebhookObservationStore),
}

impl WebhookObservationBackend {
    fn local(store: local::LocalWebhookObservationStore) -> Self {
        Self::Local(store)
    }

    fn shared(store: shared::SharedWebhookObservationStore) -> Self {
        Self::Shared(store)
    }

    fn kind(&self) -> WebhookObservationBackendKind {
        match self {
            Self::Local(_) => WebhookObservationBackendKind::LocalSqlite,
            Self::Shared(_) => WebhookObservationBackendKind::SharedPostgres,
        }
    }

    fn location_label(&self) -> String {
        match self {
            Self::Local(store) => store.location_label(),
            Self::Shared(store) => store.location_label(),
        }
    }

    fn path(&self) -> Option<&Path> {
        match self {
            Self::Local(store) => Some(store.path()),
            Self::Shared(_) => None,
        }
    }

    fn insert(&self, record: &WebhookObservationEvent) -> Result<(), String> {
        match self {
            Self::Local(store) => store.insert(record),
            Self::Shared(store) => store.insert(record),
        }
    }

    fn entry_count(&self) -> Result<usize, String> {
        match self {
            Self::Local(store) => store.count(),
            Self::Shared(store) => store.count(),
        }
    }

    fn status_counts(&self) -> Result<WebhookObservationStatusCounts, String> {
        match self {
            Self::Local(store) => store.status_counts(),
            Self::Shared(store) => store.status_counts(),
        }
    }

    fn recent(&self, limit: usize) -> Result<Vec<WebhookObservationEvent>, String> {
        match self {
            Self::Local(store) => store.recent(limit),
            Self::Shared(store) => store.recent(limit),
        }
    }

    fn claim_delivery(
        &self,
        app_id: &str,
        route_name: &str,
        source: &str,
        delivery_id: &str,
        request_id: &str,
        recorded_at_unix_seconds: i64,
    ) -> Result<bool, String> {
        match self {
            Self::Local(store) => store.claim_delivery(
                app_id,
                route_name,
                source,
                delivery_id,
                request_id,
                recorded_at_unix_seconds,
            ),
            Self::Shared(store) => store.claim_delivery(
                app_id,
                route_name,
                source,
                delivery_id,
                request_id,
                recorded_at_unix_seconds,
            ),
        }
    }
}

pub(crate) fn decode_status_counts(
    rows: impl IntoIterator<Item = (String, i64)>,
) -> Result<WebhookObservationStatusCounts, String> {
    let mut counts = WebhookObservationStatusCounts::default();
    for (status, count) in rows {
        let count = usize::try_from(count)
            .map_err(|_| "webhook observation count overflowed usize".to_string())?;
        counts.increment(WebhookObservationStatus::from_db_value(&status)?, count);
    }
    Ok(counts)
}

fn unix_seconds_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
