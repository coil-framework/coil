use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use davenda_wasm::{MetadataExecution, MetadataGrant};

use super::super::*;

mod local;
mod shared;

#[derive(Debug, Clone)]
pub(super) struct RuntimeMetadataBackend {
    backend: MetadataAuditBackend,
    write_sequence: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataAuditBackendKind {
    LocalSqlite,
    SharedPostgres,
}

impl MetadataAuditBackendKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::LocalSqlite => "local-sqlite",
            Self::SharedPostgres => "shared-postgres",
        }
    }
}

impl RuntimeMetadataBackend {
    pub(super) fn open(plan: &RuntimePlan) -> Self {
        let backend = match plan.metadata_audit_backend_selection() {
            crate::plan::MetadataAuditBackendSelection::SharedPostgres { runtime } => {
                MetadataAuditBackend::shared(shared::SharedMetadataAuditStore::open(runtime))
            }
            crate::plan::MetadataAuditBackendSelection::LocalSqlite { root, namespace } => {
                MetadataAuditBackend::local(local::LocalMetadataAuditStore::open(root, namespace))
            }
        };

        Self {
            backend,
            write_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    #[cfg(test)]
    pub(super) fn with_local_root(root: impl Into<PathBuf>, namespace: impl Into<String>) -> Self {
        Self {
            backend: MetadataAuditBackend::local(local::LocalMetadataAuditStore::open(
                root.into(),
                namespace.into(),
            )),
            write_sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn record(
        &self,
        kind: MetadataGrant,
        context: &InvocationContext,
    ) -> Result<MetadataExecution, String> {
        let record = MetadataAuditRecord::from_context(kind, context);
        self.backend.insert(&record)?;
        let journal_entries = self.write_sequence.fetch_add(1, Ordering::Relaxed) + 1;

        Ok(MetadataExecution {
            kind,
            recorded: true,
            journal_entries: journal_entries as usize,
        })
    }

    pub(super) fn snapshot(&self, limit: usize) -> Result<MetadataAuditSnapshot, String> {
        Ok(MetadataAuditSnapshot {
            backend: self.backend.kind(),
            location: self.backend.location_label(),
            path: self.backend.path().map(Path::to_path_buf),
            entry_count: self.backend.count()?,
            recent_records: self.backend.recent(limit)?,
        })
    }

    pub(super) fn backend_kind(&self) -> MetadataAuditBackendKind {
        self.backend.kind()
    }

    pub(super) fn location_label(&self) -> String {
        self.backend.location_label()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataAuditSnapshot {
    pub backend: MetadataAuditBackendKind,
    pub location: String,
    pub path: Option<PathBuf>,
    pub entry_count: usize,
    pub recent_records: Vec<MetadataAuditRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataAuditRecord {
    pub id: i64,
    pub recorded_at_unix_seconds: i64,
    pub kind: String,
    pub app_id: String,
    pub trace_id: String,
    pub request_id: Option<String>,
    pub principal_kind: String,
    pub principal_id: Option<String>,
}

impl MetadataAuditRecord {
    fn from_context(kind: MetadataGrant, context: &InvocationContext) -> Self {
        Self {
            id: 0,
            recorded_at_unix_seconds: unix_seconds_now(),
            kind: kind.to_string(),
            app_id: context.customer_app.app_id.clone(),
            trace_id: context.trace.trace_id.clone(),
            request_id: context.trace.request_id.clone(),
            principal_kind: context.principal.kind.to_string(),
            principal_id: context.principal.id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
enum MetadataAuditBackend {
    Local(local::LocalMetadataAuditStore),
    Shared(shared::SharedMetadataAuditStore),
}

impl MetadataAuditBackend {
    fn local(store: local::LocalMetadataAuditStore) -> Self {
        Self::Local(store)
    }

    fn shared(store: shared::SharedMetadataAuditStore) -> Self {
        Self::Shared(store)
    }

    fn kind(&self) -> MetadataAuditBackendKind {
        match self {
            Self::Local(_) => MetadataAuditBackendKind::LocalSqlite,
            Self::Shared(_) => MetadataAuditBackendKind::SharedPostgres,
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

    fn insert(&self, record: &MetadataAuditRecord) -> Result<(), String> {
        match self {
            Self::Local(store) => store.insert(record),
            Self::Shared(store) => store.insert(record),
        }
    }

    fn count(&self) -> Result<usize, String> {
        match self {
            Self::Local(store) => store.count(),
            Self::Shared(store) => store.count(),
        }
    }

    fn recent(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        match self {
            Self::Local(store) => store.recent(limit),
            Self::Shared(store) => store.recent(limit),
        }
    }
}

fn unix_seconds_now() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeBuilder;
    use davenda_auth::DefaultAuthModelPackage;
    use davenda_config::{PlatformConfig, StorageDeployment};
    use davenda_wasm::{
        ApiInvocation, CustomerAppContext, InvocationContext, InvocationInput, MetadataGrant,
        PrincipalRef, TraceContext,
    };
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_CONFIG: &str = r#"
[app]
name = "metadata-tests"
environment = "development"

[server]
bind = "127.0.0.1:0"
trusted_proxies = []

[http.session]
store = "memory"
idle_timeout_secs = 3600
absolute_timeout_secs = 7200

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = false
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "external"

[storage]
default_class = "public_upload"
single_node_escape_hatch = "disabled"
deployment = "single_node"
local_root = "/tmp/davenda-metadata-tests"

[cache]
l1 = "moka"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
fallback_locale = "en-GB"
localized_routes = false

[seo]
canonical_host = "example.test"
emit_json_ld = false

[auth]
package = "platform-default-auth"
explain_api = false
tenant_id = 1

[modules]
enabled = ["cms-pages"]

[wasm]
directory = "/tmp/davenda-wasm-tests"
default_time_limit_ms = 50
allow_network = true

[jobs]
backend = "redis"

[observability]
metrics = false
tracing = false

[assets]
publish_manifest = false
"#;

    #[test]
    fn runtime_metadata_backend_reports_local_sqlite_in_single_node_mode() {
        let plan = RuntimeBuilder::new(
            PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
            DefaultAuthModelPackage::default(),
        )
        .build()
        .unwrap();

        let backend = RuntimeMetadataBackend::open(&plan);

        assert_eq!(
            backend.backend_kind(),
            MetadataAuditBackendKind::LocalSqlite
        );
        assert!(backend.location_label().starts_with("local-sqlite:"));
    }

    #[test]
    fn runtime_metadata_backend_reports_shared_postgres_in_distributed_mode() {
        let mut config = PlatformConfig::from_toml_str(TEST_CONFIG).unwrap();
        config.storage.deployment = StorageDeployment::Distributed;
        let plan = RuntimeBuilder::new(config, DefaultAuthModelPackage::default())
            .build()
            .unwrap();

        let backend = RuntimeMetadataBackend::open(&plan);

        assert_eq!(
            backend.backend_kind(),
            MetadataAuditBackendKind::SharedPostgres
        );
        assert_eq!(
            backend.location_label(),
            "shared-postgres:public.metadata_audit_entries"
        );
    }

    #[test]
    fn runtime_metadata_backend_records_without_counting_the_table_on_write() {
        let root = std::env::temp_dir().join(format!(
            "davenda-metadata-audit-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let backend = RuntimeMetadataBackend::with_local_root(root.clone(), "metadata-tests");
        let context = InvocationContext::new(
            CustomerAppContext::new("metadata-tests")
                .unwrap()
                .with_tenant_id("1")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            TraceContext::new("trace-metadata").unwrap(),
            InvocationInput::Api(
                ApiInvocation::new("/metadata", davenda_wasm::HttpMethod::Get).unwrap(),
            ),
        );

        let first = backend.record(MetadataGrant::JsonLd, &context).unwrap();
        let second = backend.record(MetadataGrant::JsonLd, &context).unwrap();
        let snapshot = backend.snapshot(10).unwrap();

        assert_eq!(first.journal_entries, 1);
        assert_eq!(second.journal_entries, 2);
        assert_eq!(snapshot.entry_count, 2);

        fs::remove_dir_all(root).unwrap();
    }
}
