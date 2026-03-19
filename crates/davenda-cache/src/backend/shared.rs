use super::{CacheBackendKind, CacheBackendState, DistributedCacheRuntime};
use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError, FillDecision,
    FillLease, InvalidationSet, RequestCoalescingMode,
};
use bincode::{deserialize, serialize};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
#[cfg(not(test))]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const SHARED_STATE_DIR_ENV: &str = "DAVENDA_SHARED_STATE_DIR";
#[cfg(not(test))]
const SHARED_STATE_NAMESPACE_ENV: &str = "DAVENDA_SHARED_BACKEND_NAMESPACE";

#[cfg(not(test))]
static NAMESPACE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn shared_runtime(_kind: CacheBackendKind) -> Arc<dyn DistributedCacheRuntime> {
    #[cfg(test)]
    {
        Arc::new(super::EmulatedDistributedCacheRuntime::new())
    }

    #[cfg(not(test))]
    {
        persistent_runtime(_kind, default_namespace(_kind))
    }
}

pub(crate) fn persistent_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
) -> Arc<dyn DistributedCacheRuntime> {
    Arc::new(PersistentDistributedCacheRuntime::new(
        kind,
        namespace.into(),
    ))
}

#[cfg(not(test))]
fn default_namespace(kind: CacheBackendKind) -> String {
    if let Ok(namespace) = std::env::var(SHARED_STATE_NAMESPACE_ENV) {
        return namespace;
    }

    format!(
        "{}-{}-{}",
        cache_kind_slug(kind),
        std::process::id(),
        NAMESPACE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn cache_kind_slug(kind: CacheBackendKind) -> &'static str {
    match kind {
        CacheBackendKind::Local => "local",
        CacheBackendKind::Redis => "redis",
        CacheBackendKind::Valkey => "valkey",
    }
}

fn shared_state_root() -> PathBuf {
    std::env::var_os(SHARED_STATE_DIR_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("davenda-shared"))
}

fn database_path(kind: CacheBackendKind) -> PathBuf {
    shared_state_root()
        .join("cache")
        .join(format!("{}.sqlite3", cache_kind_slug(kind)))
}

#[derive(Debug)]
struct PersistentDistributedCacheRuntime {
    store: SharedCacheStore,
}

impl PersistentDistributedCacheRuntime {
    fn new(kind: CacheBackendKind, namespace: String) -> Self {
        Self {
            store: SharedCacheStore::open(kind, namespace),
        }
    }
}

impl DistributedCacheRuntime for PersistentDistributedCacheRuntime {
    fn insert(&self, entry: CacheEntry) {
        self.store
            .with_state_mut(|state| {
                state.insert(entry);
                Ok(())
            })
            .expect("persistent cache backend insert failed");
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.store
            .with_state_mut(|state| Ok(state.lookup(key, now)))
            .expect("persistent cache backend lookup failed")
    }

    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.store
            .with_state_mut(|state| Ok(state.invalidate(tags)))
            .expect("persistent cache backend invalidation failed")
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        self.store
            .with_state_mut(|state| Ok(state.begin_fill(key, mode, holder)))
            .expect("persistent cache backend fill coordination failed")
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.store
            .with_state_mut(|state| state.complete_fill(lease))
    }

    fn metrics(&self) -> CacheMetrics {
        self.store
            .read_state(|state| state.metrics)
            .expect("persistent cache backend metrics read failed")
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}

#[derive(Debug)]
struct SharedCacheStore {
    connection: Mutex<Connection>,
    namespace: String,
}

impl SharedCacheStore {
    fn open(kind: CacheBackendKind, namespace: String) -> Self {
        let path = database_path(kind);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create persistent cache backend directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open persistent cache backend `{}`: {error}",
                path.display()
            )
        });
        connection
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                CREATE TABLE IF NOT EXISTS cache_state (
                    namespace TEXT PRIMARY KEY,
                    payload BLOB NOT NULL
                );
                "#,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to initialize persistent cache backend `{}`: {error}",
                    path.display()
                )
            });

        Self {
            connection: Mutex::new(connection),
            namespace,
        }
    }

    fn read_state<T>(
        &self,
        op: impl FnOnce(&CacheBackendState) -> T,
    ) -> Result<T, CacheModelError> {
        let connection = self
            .connection
            .lock()
            .expect("cache backend mutex poisoned");
        let payload = connection
            .query_row(
                "SELECT payload FROM cache_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read persistent cache backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let state = match payload {
            Some(payload) => deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize persistent cache backend state for `{}`: {error}",
                    self.namespace
                )
            }),
            None => CacheBackendState::new(),
        };

        Ok(op(&state))
    }

    fn with_state_mut<T>(
        &self,
        op: impl FnOnce(&mut CacheBackendState) -> Result<T, CacheModelError>,
    ) -> Result<T, CacheModelError> {
        let mut connection = self
            .connection
            .lock()
            .expect("cache backend mutex poisoned");
        let tx = connection.transaction().unwrap_or_else(|error| {
            panic!(
                "failed to start persistent cache backend transaction for `{}`: {error}",
                self.namespace
            )
        });
        let payload = tx
            .query_row(
                "SELECT payload FROM cache_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read persistent cache backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let mut state = match payload {
            Some(payload) => deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize persistent cache backend state for `{}`: {error}",
                    self.namespace
                )
            }),
            None => CacheBackendState::new(),
        };

        let outcome = op(&mut state);
        if outcome.is_ok() {
            let payload = serialize(&state).unwrap_or_else(|error| {
                panic!(
                    "failed to serialize persistent cache backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.execute(
                "INSERT INTO cache_state (namespace, payload) VALUES (?1, ?2)
                 ON CONFLICT(namespace) DO UPDATE SET payload = excluded.payload",
                params![self.namespace.as_str(), payload],
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to persist cache backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.commit().unwrap_or_else(|error| {
                panic!(
                    "failed to commit persistent cache backend state for `{}`: {error}",
                    self.namespace
                )
            });
        }

        outcome
    }
}
