use std::path::{Path, PathBuf};

use davenda_data::DataRuntime;
use davenda_wasm::{MetadataExecution, MetadataGrant};

use super::super::*;

mod postgres;
mod sqlite;

#[derive(Debug, Clone)]
pub(super) struct RuntimeMetadataBackend {
    store: MetadataAuditStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetadataAuditBackendKind {
    Sqlite,
    Postgres,
}

impl MetadataAuditBackendKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
        }
    }
}

impl RuntimeMetadataBackend {
    pub(super) fn open(plan: &RuntimePlan) -> Self {
        let store = match plan.config.storage.deployment {
            davenda_config::StorageDeployment::Distributed => {
                MetadataAuditStore::postgres(plan.data.clone())
            }
            davenda_config::StorageDeployment::SingleNode => MetadataAuditStore::sqlite(
                PathBuf::from(&plan.config.storage.local_root),
                plan.shared_backend_namespace(),
            ),
        };

        Self { store }
    }

    #[cfg(test)]
    pub(super) fn with_root(root: impl Into<PathBuf>, namespace: impl Into<String>) -> Self {
        Self {
            store: MetadataAuditStore::sqlite(root.into(), namespace.into()),
        }
    }

    pub(super) fn record(
        &self,
        kind: MetadataGrant,
        context: &InvocationContext,
    ) -> Result<MetadataExecution, String> {
        let record = MetadataAuditRecord::from_context(kind, context);
        self.store.insert(&record)?;
        let journal_entries = self.store.count()?;

        Ok(MetadataExecution {
            kind,
            recorded: true,
            journal_entries,
        })
    }

    pub(super) fn snapshot(&self, limit: usize) -> Result<MetadataAuditSnapshot, String> {
        Ok(MetadataAuditSnapshot {
            backend: self.store.kind(),
            location: self.store.location_label(),
            path: self.store.path().map(Path::to_path_buf),
            entry_count: self.store.count()?,
            recent_records: self.store.recent(limit)?,
        })
    }

    #[cfg(test)]
    pub(super) fn recent_records(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        self.store.recent(limit)
    }

    pub(super) fn backend_kind(&self) -> MetadataAuditBackendKind {
        self.store.kind()
    }

    pub(super) fn location_label(&self) -> String {
        self.store.location_label()
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
enum MetadataAuditStore {
    Sqlite(sqlite::SqliteMetadataAuditStore),
    Postgres(postgres::PostgresMetadataAuditStore),
}

impl MetadataAuditStore {
    fn sqlite(root: PathBuf, namespace: String) -> Self {
        Self::Sqlite(sqlite::SqliteMetadataAuditStore::open(root, namespace))
    }

    fn postgres(runtime: DataRuntime) -> Self {
        Self::Postgres(postgres::PostgresMetadataAuditStore::open(runtime))
    }

    fn kind(&self) -> MetadataAuditBackendKind {
        match self {
            Self::Sqlite(_) => MetadataAuditBackendKind::Sqlite,
            Self::Postgres(_) => MetadataAuditBackendKind::Postgres,
        }
    }

    fn location_label(&self) -> String {
        match self {
            Self::Sqlite(store) => store.location_label(),
            Self::Postgres(store) => store.location_label(),
        }
    }

    fn path(&self) -> Option<&Path> {
        match self {
            Self::Sqlite(store) => Some(store.path()),
            Self::Postgres(_) => None,
        }
    }

    fn insert(&self, record: &MetadataAuditRecord) -> Result<(), String> {
        match self {
            Self::Sqlite(store) => store.insert(record),
            Self::Postgres(store) => store.insert(record),
        }
    }

    fn count(&self) -> Result<usize, String> {
        match self {
            Self::Sqlite(store) => store.count(),
            Self::Postgres(store) => store.count(),
        }
    }

    fn recent(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        match self {
            Self::Sqlite(store) => store.recent(limit),
            Self::Postgres(store) => store.recent(limit),
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
    use davenda_config::PlatformConfig;

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
    fn runtime_metadata_backend_reports_sqlite_in_single_node_mode() {
        let plan = RuntimeBuilder::new(
            PlatformConfig::from_toml_str(TEST_CONFIG).unwrap(),
            DefaultAuthModelPackage::default(),
        )
        .build()
        .unwrap();

        let backend = RuntimeMetadataBackend::open(&plan);

        assert_eq!(backend.backend_kind(), MetadataAuditBackendKind::Sqlite);
        assert!(backend.location_label().starts_with("sqlite:"));
    }
}
