use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use davenda_data::{DataRuntime, PostgresDataClient};
use davenda_wasm::{MetadataExecution, MetadataGrant};
use rusqlite::{Connection, params};
use sqlx::Row;

use super::super::*;

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
    Sqlite(SqliteMetadataAuditStore),
    Postgres(PostgresMetadataAuditStore),
}

impl MetadataAuditStore {
    fn sqlite(root: PathBuf, namespace: String) -> Self {
        Self::Sqlite(SqliteMetadataAuditStore::open(root, namespace))
    }

    fn postgres(runtime: DataRuntime) -> Self {
        Self::Postgres(PostgresMetadataAuditStore::open(runtime))
    }

    fn kind(&self) -> MetadataAuditBackendKind {
        match self {
            Self::Sqlite(_) => MetadataAuditBackendKind::Sqlite,
            Self::Postgres(_) => MetadataAuditBackendKind::Postgres,
        }
    }

    fn location_label(&self) -> String {
        match self {
            Self::Sqlite(store) => store.path.display().to_string(),
            Self::Postgres(store) => store.location_label(),
        }
    }

    fn path(&self) -> Option<&Path> {
        match self {
            Self::Sqlite(store) => Some(store.path.as_path()),
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

#[derive(Debug, Clone)]
struct SqliteMetadataAuditStore {
    path: PathBuf,
    connection: std::sync::Arc<Mutex<Connection>>,
}

impl SqliteMetadataAuditStore {
    fn open(root: PathBuf, namespace: String) -> Self {
        let path = database_path(&root, &namespace);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create metadata audit directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open metadata audit store `{}`: {error}",
                path.display()
            )
        });
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                CREATE TABLE IF NOT EXISTS metadata_audit_entries (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    recorded_at_unix_seconds INTEGER NOT NULL,
                    app_id TEXT NOT NULL,
                    trace_id TEXT NOT NULL,
                    request_id TEXT,
                    principal_kind TEXT NOT NULL,
                    principal_id TEXT,
                    kind TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS metadata_audit_entries_recent
                    ON metadata_audit_entries (recorded_at_unix_seconds DESC, id DESC);
                "#,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to initialize metadata audit store `{}`: {error}",
                    path.display()
                )
            });

        Self {
            path,
            connection: std::sync::Arc::new(Mutex::new(connection)),
        }
    }

    fn insert(&self, record: &MetadataAuditRecord) -> Result<(), String> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| "metadata audit store is poisoned".to_string())?;
        let tx = connection
            .transaction()
            .map_err(|error| format!("failed to start metadata audit transaction: {error}"))?;
        tx.execute(
            r#"
            INSERT INTO metadata_audit_entries (
                recorded_at_unix_seconds,
                app_id,
                trace_id,
                request_id,
                principal_kind,
                principal_id,
                kind
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                record.recorded_at_unix_seconds,
                record.app_id,
                record.trace_id,
                record.request_id,
                record.principal_kind,
                record.principal_id,
                record.kind,
            ],
        )
        .map_err(|error| format!("failed to write metadata audit entry: {error}"))?;
        tx.commit()
            .map_err(|error| format!("failed to commit metadata audit entry: {error}"))?;
        Ok(())
    }

    fn count(&self) -> Result<usize, String> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| "metadata audit store is poisoned".to_string())?;
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM metadata_audit_entries", [], |row| {
                row.get(0)
            })
            .map_err(|error| format!("failed to count metadata audit entries: {error}"))?;
        usize::try_from(count)
            .map_err(|_| "metadata audit entry count overflowed usize".to_string())
    }

    fn recent(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let connection = self
            .connection
            .lock()
            .map_err(|_| "metadata audit store is poisoned".to_string())?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT
                    id,
                    recorded_at_unix_seconds,
                    app_id,
                    trace_id,
                    request_id,
                    principal_kind,
                    principal_id,
                    kind
                FROM metadata_audit_entries
                ORDER BY recorded_at_unix_seconds DESC, id DESC
                LIMIT ?1
                "#,
            )
            .map_err(|error| format!("failed to query metadata audit entries: {error}"))?;

        let mut records = statement
            .query_map(params![limit as i64], |row| {
                Ok(MetadataAuditRecord {
                    id: row.get(0)?,
                    recorded_at_unix_seconds: row.get(1)?,
                    app_id: row.get(2)?,
                    trace_id: row.get(3)?,
                    request_id: row.get(4)?,
                    principal_kind: row.get(5)?,
                    principal_id: row.get(6)?,
                    kind: row.get(7)?,
                })
            })
            .map_err(|error| format!("failed to map metadata audit entries: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("failed to collect metadata audit entries: {error}"))?;
        records.reverse();
        Ok(records)
    }
}

#[derive(Debug, Clone)]
struct PostgresMetadataAuditStore {
    runtime: DataRuntime,
    client: OnceLock<Result<PostgresDataClient, String>>,
    schema: String,
    initialized: OnceLock<Result<(), String>>,
}

impl PostgresMetadataAuditStore {
    fn open(runtime: DataRuntime) -> Self {
        let schema = runtime.schema.clone();
        Self {
            runtime,
            client: OnceLock::new(),
            schema,
            initialized: OnceLock::new(),
        }
    }

    fn location_label(&self) -> String {
        format!("{}.metadata_audit_entries", self.schema)
    }

    fn insert(&self, record: &MetadataAuditRecord) -> Result<(), String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            sqlx::query(&format!(
                "INSERT INTO {} (recorded_at_unix_seconds, app_id, trace_id, request_id, principal_kind, principal_id, kind) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                table
            ))
            .bind(record.recorded_at_unix_seconds)
            .bind(&record.app_id)
            .bind(&record.trace_id)
            .bind(&record.request_id)
            .bind(&record.principal_kind)
            .bind(&record.principal_id)
            .bind(&record.kind)
            .execute(&client.pool)
            .await
            .map_err(|error| format!("failed to write metadata audit entry: {error}"))?;
            Ok(())
        })
    }

    fn count(&self) -> Result<usize, String> {
        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", table))
            .fetch_one(&client.pool)
            .await
            .map_err(|error| format!("failed to count metadata audit entries: {error}"))?;
            usize::try_from(count)
                .map_err(|_| "metadata audit entry count overflowed usize".to_string())
        })
    }

    fn recent(&self, limit: usize) -> Result<Vec<MetadataAuditRecord>, String> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        self.ensure_initialized()?;
        let client = self.client()?.clone();
        let table = self.qualified_table();
        run_blocking(async move {
            let rows = sqlx::query(&format!(
                "SELECT id, recorded_at_unix_seconds, app_id, trace_id, request_id, principal_kind, principal_id, kind FROM {} ORDER BY recorded_at_unix_seconds DESC, id DESC LIMIT $1",
                table
            ))
            .bind(limit as i64)
            .fetch_all(&client.pool)
            .await
            .map_err(|error| format!("failed to query metadata audit entries: {error}"))?;

            let mut records = rows
                .into_iter()
                .map(|row| {
                    Ok(MetadataAuditRecord {
                        id: row
                            .try_get(0)
                            .map_err(|error| format!("failed to decode metadata audit entry id: {error}"))?,
                        recorded_at_unix_seconds: row
                            .try_get(1)
                            .map_err(|error| format!("failed to decode metadata audit timestamp: {error}"))?,
                        app_id: row
                            .try_get(2)
                            .map_err(|error| format!("failed to decode metadata audit app id: {error}"))?,
                        trace_id: row
                            .try_get(3)
                            .map_err(|error| format!("failed to decode metadata audit trace id: {error}"))?,
                        request_id: row
                            .try_get(4)
                            .map_err(|error| format!("failed to decode metadata audit request id: {error}"))?,
                        principal_kind: row
                            .try_get(5)
                            .map_err(|error| format!("failed to decode metadata audit principal kind: {error}"))?,
                        principal_id: row
                            .try_get(6)
                            .map_err(|error| format!("failed to decode metadata audit principal id: {error}"))?,
                        kind: row
                            .try_get(7)
                            .map_err(|error| format!("failed to decode metadata audit kind: {error}"))?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            records.reverse();
            Ok(records)
        })
    }

    fn client(&self) -> Result<&PostgresDataClient, String> {
        self.client
            .get_or_init(|| self.runtime.connect_lazy_postgres().map_err(|error| error.to_string()))
            .as_ref()
            .map_err(|error| error.clone())
    }

    fn ensure_initialized(&self) -> Result<(), String> {
        let schema_ident = quote_identifier(&self.schema);
        self.initialized
            .get_or_init(|| {
                let client = self.client()?.clone();
                run_blocking(async move {
                    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS {schema_ident}"))
                        .execute(&client.pool)
                        .await
                        .map_err(|error| format!("failed to initialize metadata schema: {error}"))?;

                    sqlx::query(&format!(
                        "CREATE TABLE IF NOT EXISTS {schema_ident}.metadata_audit_entries (
                            id BIGSERIAL PRIMARY KEY,
                            recorded_at_unix_seconds BIGINT NOT NULL,
                            app_id TEXT NOT NULL,
                            trace_id TEXT NOT NULL,
                            request_id TEXT,
                            principal_kind TEXT NOT NULL,
                            principal_id TEXT,
                            kind TEXT NOT NULL
                        )"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!("failed to initialize metadata audit table: {error}"))?;

                    sqlx::query(&format!(
                        "CREATE INDEX IF NOT EXISTS metadata_audit_entries_recent
                            ON {schema_ident}.metadata_audit_entries (recorded_at_unix_seconds DESC, id DESC)"
                    ))
                    .execute(&client.pool)
                    .await
                    .map_err(|error| format!("failed to initialize metadata audit index: {error}"))?;

                    Ok(())
                })
            })
            .clone()
    }

    fn qualified_table(&self) -> String {
        format!(
            "{}.{}",
            quote_identifier(&self.schema),
            quote_identifier("metadata_audit_entries")
        )
    }
}

fn database_path(root: &Path, namespace: &str) -> PathBuf {
    root.join("wasm")
        .join("metadata")
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

fn sanitize_namespace(namespace: &str) -> String {
    namespace
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn run_blocking<T, F>(future: F) -> Result<T, String>
where
    F: Future<Output = Result<T, String>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        runtime.block_on(future)
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn execution_context(trace_id: &str, request_id: Option<&str>) -> InvocationContext {
        let trace = if let Some(request_id) = request_id {
            TraceContext::new(trace_id)
                .unwrap()
                .with_request_id(request_id)
                .unwrap()
        } else {
            TraceContext::new(trace_id).unwrap()
        };

        InvocationContext::new(
            CustomerAppContext::new("audit-app")
                .unwrap()
                .with_tenant_id("101")
                .unwrap()
                .with_locale("en-GB")
                .unwrap(),
            PrincipalRef::user("alice").unwrap(),
            trace,
            InvocationInput::Page(
                PageInvocation::new("/metadata", davenda_wasm::HttpMethod::Get).unwrap(),
            ),
        )
    }

    fn shared_state_root(label: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("davenda-metadata-{}-{}", std::process::id(), label));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn runtime_metadata_backend_persists_and_queries_audit_records() {
        let root = shared_state_root("persistence");
        let backend = RuntimeMetadataBackend::with_root(root.clone(), "audit-suite");

        let first = backend
            .record(
                MetadataGrant::JsonLd,
                &execution_context("trace-1", Some("req-1")),
            )
            .unwrap();
        assert_eq!(first.journal_entries, 1);

        let second = backend
            .record(
                MetadataGrant::SeoHead,
                &execution_context("trace-2", Some("req-2")),
            )
            .unwrap();
        assert_eq!(second.journal_entries, 2);

        let reopened = RuntimeMetadataBackend::with_root(root, "audit-suite");
        let snapshot = reopened.snapshot(10).unwrap();
        assert_eq!(snapshot.entry_count, 2);
        assert!(snapshot.path.as_ref().unwrap().exists());
        let records = reopened.recent_records(10).unwrap();

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].kind, "json_ld");
        assert_eq!(records[0].trace_id, "trace-1");
        assert_eq!(records[0].request_id.as_deref(), Some("req-1"));
        assert_eq!(records[0].principal_kind, "user");
        assert_eq!(records[1].kind, "seo_head");
        assert_eq!(records[1].trace_id, "trace-2");
        assert_eq!(records[1].request_id.as_deref(), Some("req-2"));

        let last_only = reopened.recent_records(1).unwrap();
        assert_eq!(last_only.len(), 1);
        assert_eq!(last_only[0].kind, "seo_head");
    }
}
