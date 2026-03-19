#[cfg(test)]
use super::EmulatedDistributedCacheRuntime;
use super::{CacheBackendKind, CacheBackendState, DistributedCacheRuntime};
use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError, FillDecision,
    FillLease, InvalidationSet, RequestCoalescingMode,
};
use bincode::{deserialize, serialize};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::PathBuf;
#[cfg(test)]
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};

const SHARED_STATE_DIR_ENV: &str = "DAVENDA_SHARED_STATE_DIR";

#[cfg(test)]
pub(crate) fn persistent_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
) -> Arc<dyn DistributedCacheRuntime> {
    shared_test_runtime(kind, namespace.into())
}

#[cfg(test)]
fn shared_test_runtime(
    kind: CacheBackendKind,
    namespace: String,
) -> Arc<dyn DistributedCacheRuntime> {
    static REGISTRY: OnceLock<
        Mutex<std::collections::BTreeMap<String, Arc<dyn DistributedCacheRuntime>>>,
    > = OnceLock::new();

    let key = format!("{}:{kind:?}:{namespace}", test_scope());
    let registry = REGISTRY.get_or_init(|| Mutex::new(std::collections::BTreeMap::new()));
    let mut guard = registry.lock().expect("test cache registry mutex poisoned");
    guard
        .entry(key)
        .or_insert_with(|| {
            Arc::new(SharedCacheRuntimeHarness::new(Arc::new(
                EmulatedDistributedCacheRuntime::new(),
            )))
        })
        .clone()
}

#[cfg(not(test))]
pub(crate) fn persistent_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
) -> Arc<dyn DistributedCacheRuntime> {
    Arc::new(PersistentDistributedCacheRuntime::new(
        kind,
        namespace.into(),
    ))
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

fn database_path(kind: CacheBackendKind, namespace: &str) -> PathBuf {
    shared_state_root()
        .join("cache")
        .join(cache_kind_slug(kind))
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
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
        let path = database_path(kind, &namespace);
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

#[cfg(test)]
fn test_scope() -> String {
    std::thread::current()
        .name()
        .unwrap_or("unnamed-test")
        .to_string()
}

#[cfg(test)]
#[derive(Clone)]
struct SharedCacheRuntimeHarness {
    runtime: Arc<dyn DistributedCacheRuntime>,
}

#[cfg(test)]
impl SharedCacheRuntimeHarness {
    fn new(runtime: Arc<dyn DistributedCacheRuntime>) -> Self {
        Self { runtime }
    }
}

#[cfg(test)]
impl DistributedCacheRuntime for SharedCacheRuntimeHarness {
    fn insert(&self, entry: CacheEntry) {
        self.runtime.insert(entry);
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.runtime.lookup(key, now)
    }

    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.runtime.invalidate(tags)
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        self.runtime.begin_fill(key, mode, holder)
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.runtime.complete_fill(lease)
    }

    fn metrics(&self) -> CacheMetrics {
        self.runtime.metrics()
    }

    fn is_shared_backend(&self) -> bool {
        true
    }
}
