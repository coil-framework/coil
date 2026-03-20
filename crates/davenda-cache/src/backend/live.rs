use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[cfg(not(test))]
use std::env;

#[cfg(not(test))]
use redis::Commands;
#[cfg(test)]
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::state::CacheBackendState;
use super::{CacheBackendKind, DistributedCacheRuntime};
use crate::{
    CacheEntry, CacheInstant, CacheKey, CacheLookup, CacheMetrics, CacheModelError, FillDecision,
    FillLease, InvalidationSet, RequestCoalescingMode,
};

#[cfg(not(test))]
pub fn live_shared_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
    _root: impl Into<PathBuf>,
) -> Arc<dyn DistributedCacheRuntime> {
    Arc::new(ProductionRedisSharedCacheRuntime::new(
        kind,
        namespace.into(),
    ))
}

#[cfg(not(test))]
struct ProductionRedisSharedCacheRuntime {
    store: ProductionRedisSharedCacheStore,
}

#[cfg(not(test))]
impl ProductionRedisSharedCacheRuntime {
    fn new(kind: CacheBackendKind, namespace: String) -> Self {
        Self {
            store: ProductionRedisSharedCacheStore::open(kind, namespace),
        }
    }
}

#[cfg(not(test))]
impl DistributedCacheRuntime for ProductionRedisSharedCacheRuntime {
    fn insert(&self, entry: CacheEntry) {
        self.store
            .with_state_mut(|state| {
                state.insert(entry);
                Ok(())
            })
            .expect("redis cache backend insert failed");
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.store
            .with_state_mut(|state| Ok(state.lookup(key, now)))
            .expect("redis cache backend lookup failed")
    }

    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.store
            .with_state_mut(|state| Ok(state.invalidate(tags)))
            .expect("redis cache backend invalidation failed")
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        self.store
            .with_state_mut(|state| Ok(state.begin_fill(key, mode, holder)))
            .expect("redis cache backend fill coordination failed")
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.store
            .with_state_mut(|state| state.complete_fill(lease))
    }

    fn metrics(&self) -> CacheMetrics {
        self.store
            .read_state(|state| state.metrics())
            .expect("redis cache backend metrics read failed")
    }

    fn is_shared_backend(&self) -> bool {
        true
    }

    fn supports_live_shared_state(&self) -> bool {
        true
    }
}

#[cfg(not(test))]
struct ProductionRedisSharedCacheStore {
    connection: Mutex<redis::Connection>,
    key: String,
}

#[cfg(not(test))]
impl ProductionRedisSharedCacheStore {
    fn open(kind: CacheBackendKind, namespace: String) -> Self {
        let url = cache_backend_url(kind);
        let client = redis::Client::open(url.as_str())
            .unwrap_or_else(|error| panic!("failed to open redis cache backend `{url}`: {error}"));
        let connection = client.get_connection().unwrap_or_else(|error| {
            panic!("failed to connect to redis cache backend `{url}`: {error}")
        });
        Self {
            connection: Mutex::new(connection),
            key: format!("davenda:cache:{kind:?}:{namespace}"),
        }
    }

    fn read_state<T>(
        &self,
        op: impl FnOnce(&CacheBackendState) -> T,
    ) -> Result<T, CacheModelError> {
        let mut connection = self
            .connection
            .lock()
            .expect("redis cache backend mutex poisoned");
        let payload: Option<Vec<u8>> = connection
            .get(&self.key)
            .unwrap_or_else(|error| panic!("failed to read redis cache backend state: {error}"));
        let state = match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!("failed to deserialize redis cache backend state: {error}")
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
            .expect("redis cache backend mutex poisoned");
        let payload: Option<Vec<u8>> = connection
            .get(&self.key)
            .unwrap_or_else(|error| panic!("failed to read redis cache backend state: {error}"));
        let mut state = match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!("failed to deserialize redis cache backend state: {error}")
            }),
            None => CacheBackendState::new(),
        };
        let outcome = op(&mut state);
        if outcome.is_ok() {
            let serialized = bincode::serialize(&state).unwrap_or_else(|error| {
                panic!("failed to serialize redis cache backend state: {error}")
            });
            connection
                .set::<_, _, ()>(&self.key, serialized)
                .unwrap_or_else(|error| {
                    panic!("failed to persist redis cache backend state: {error}")
                });
        }
        outcome
    }
}

#[cfg(not(test))]
fn cache_backend_url(kind: CacheBackendKind) -> String {
    match kind {
        CacheBackendKind::Redis => {
            env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string())
        }
        CacheBackendKind::Valkey => env::var("VALKEY_URL")
            .or_else(|_| env::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1/".to_string()),
        CacheBackendKind::Local => {
            panic!("local cache backends are test-only and cannot back a live shared runtime")
        }
    }
}

#[cfg(test)]
pub fn live_shared_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
    root: impl Into<PathBuf>,
) -> Arc<dyn DistributedCacheRuntime> {
    Arc::new(LiveSharedCacheRuntime::new(
        kind,
        namespace.into(),
        root.into(),
    ))
}

#[derive(Debug)]
#[cfg(test)]
struct LiveSharedCacheRuntime {
    store: LiveSharedCacheStore,
}

#[cfg(test)]
impl LiveSharedCacheRuntime {
    fn new(kind: CacheBackendKind, namespace: String, root: PathBuf) -> Self {
        Self {
            store: LiveSharedCacheStore::open(kind, namespace, root),
        }
    }
}

#[cfg(test)]
impl DistributedCacheRuntime for LiveSharedCacheRuntime {
    fn insert(&self, entry: CacheEntry) {
        self.store
            .with_state_mut(|state| {
                state.insert(entry);
                Ok(())
            })
            .expect("live cache backend insert failed");
    }

    fn lookup(&self, key: &CacheKey, now: CacheInstant) -> CacheLookup {
        self.store
            .with_state_mut(|state| Ok(state.lookup(key, now)))
            .expect("live cache backend lookup failed")
    }

    fn invalidate(&self, tags: &InvalidationSet) -> Vec<CacheKey> {
        self.store
            .with_state_mut(|state| Ok(state.invalidate(tags)))
            .expect("live cache backend invalidation failed")
    }

    fn begin_fill(
        &self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: String,
    ) -> FillDecision {
        self.store
            .with_state_mut(|state| Ok(state.begin_fill(key, mode, holder)))
            .expect("live cache backend fill coordination failed")
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        self.store
            .with_state_mut(|state| state.complete_fill(lease))
    }

    fn metrics(&self) -> CacheMetrics {
        self.store
            .read_state(|state| state.metrics())
            .expect("live cache backend metrics read failed")
    }

    fn is_shared_backend(&self) -> bool {
        true
    }

    fn supports_live_shared_state(&self) -> bool {
        true
    }
}

#[derive(Debug)]
#[cfg(test)]
struct LiveSharedCacheStore {
    connection: Mutex<Connection>,
    namespace: String,
}

#[cfg(test)]
impl LiveSharedCacheStore {
    fn open(kind: CacheBackendKind, namespace: String, root: PathBuf) -> Self {
        let path = live_database_path(kind, &namespace, root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|error| {
                panic!(
                    "failed to create live cache backend directory `{}`: {error}",
                    parent.display()
                )
            });
        }

        let connection = Connection::open(&path).unwrap_or_else(|error| {
            panic!(
                "failed to open live cache backend `{}`: {error}",
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
                    "failed to initialize live cache backend `{}`: {error}",
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
            .expect("live cache backend mutex poisoned");
        let payload = connection
            .query_row(
                "SELECT payload FROM cache_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row: &Row<'_>| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read live cache backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let state = match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize live cache backend state for `{}`: {error}",
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
            .expect("live cache backend mutex poisoned");
        let tx = connection.transaction().unwrap_or_else(|error| {
            panic!(
                "failed to start live cache backend transaction for `{}`: {error}",
                self.namespace
            )
        });
        let payload = tx
            .query_row(
                "SELECT payload FROM cache_state WHERE namespace = ?1",
                params![self.namespace.as_str()],
                |row: &Row<'_>| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read live cache backend state for `{}`: {error}",
                    self.namespace
                )
            });

        let mut state = match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!(
                    "failed to deserialize live cache backend state for `{}`: {error}",
                    self.namespace
                )
            }),
            None => CacheBackendState::new(),
        };

        let outcome = op(&mut state);
        if outcome.is_ok() {
            let payload = bincode::serialize(&state).unwrap_or_else(|error| {
                panic!(
                    "failed to serialize live cache backend state for `{}`: {error}",
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
                    "failed to persist live cache backend state for `{}`: {error}",
                    self.namespace
                )
            });
            tx.commit().unwrap_or_else(|error| {
                panic!(
                    "failed to commit live cache backend state for `{}`: {error}",
                    self.namespace
                )
            });
        }

        outcome
    }
}

#[cfg(test)]
fn live_database_path(kind: CacheBackendKind, namespace: &str, root: PathBuf) -> PathBuf {
    root.join("cache")
        .join(cache_kind_slug(kind))
        .join(format!("{}.sqlite3", sanitize_namespace(namespace)))
}

#[cfg(test)]
fn cache_kind_slug(kind: CacheBackendKind) -> &'static str {
    match kind {
        CacheBackendKind::Local => "local",
        CacheBackendKind::Redis => "redis",
        CacheBackendKind::Valkey => "valkey",
    }
}

#[cfg(test)]
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
