use std::path::PathBuf;
use std::sync::Arc;

#[cfg(not(test))]
use std::sync::Mutex;

#[cfg(not(test))]
use redis::Commands;

#[cfg(not(test))]
use super::state::CacheBackendState;
use super::{CacheBackendKind, DistributedCacheRuntime};
#[cfg(not(test))]
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

#[cfg(test)]
pub fn live_shared_runtime(
    kind: CacheBackendKind,
    namespace: impl Into<String>,
    _root: impl Into<PathBuf>,
) -> Arc<dyn DistributedCacheRuntime> {
    super::testing::test_only_sqlite_shared_runtime(kind, namespace.into())
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
        let entry = entry.clone();
        self.store
            .with_state_mut(move |state| {
                state.insert(entry.clone());
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
        let key = key.clone();
        let holder = holder.clone();
        self.store
            .with_state_mut(move |state| Ok(state.begin_fill(&key, mode, holder.clone())))
            .expect("redis cache backend fill coordination failed")
    }

    fn complete_fill(&self, lease: &FillLease) -> Result<(), CacheModelError> {
        let lease = lease.clone();
        self.store
            .with_state_mut(move |state| state.complete_fill(&lease))
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
        let url = cache_backend_url(
            kind,
            std::env::var("REDIS_URL").ok(),
            std::env::var("VALKEY_URL").ok(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
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
        let state = Self::load_state(&mut connection, &self.key);
        Ok(op(&state))
    }

    fn with_state_mut<T>(
        &self,
        mut op: impl FnMut(&mut CacheBackendState) -> Result<T, CacheModelError>,
    ) -> Result<T, CacheModelError> {
        let mut connection = self
            .connection
            .lock()
            .expect("redis cache backend mutex poisoned");
        redis::transaction(
            &mut *connection,
            &[self.key.as_str()],
            |connection, pipeline| {
                let mut state = Self::load_state(connection, &self.key);
                let outcome = op(&mut state);
                if outcome.is_ok() {
                    pipeline
                        .set(&self.key, Self::serialize_state(&state))
                        .ignore()
                        .query::<()>(connection)
                        .unwrap_or_else(|error| {
                            panic!("failed to persist redis cache backend state: {error}")
                        });
                }
                Ok(Some(outcome))
            },
        )
        .unwrap_or_else(|error| panic!("failed to coordinate redis cache backend state: {error}"))
    }

    fn load_state(connection: &mut redis::Connection, key: &str) -> CacheBackendState {
        let payload: Option<Vec<u8>> = connection
            .get(key)
            .unwrap_or_else(|error| panic!("failed to read redis cache backend state: {error}"));
        match payload {
            Some(payload) => bincode::deserialize(&payload).unwrap_or_else(|error| {
                panic!("failed to deserialize redis cache backend state: {error}")
            }),
            None => CacheBackendState::new(),
        }
    }

    fn serialize_state(state: &CacheBackendState) -> Vec<u8> {
        bincode::serialize(state).unwrap_or_else(|error| {
            panic!("failed to serialize redis cache backend state: {error}")
        })
    }
}

fn cache_backend_url(
    kind: CacheBackendKind,
    redis_url: Option<String>,
    valkey_url: Option<String>,
) -> Result<String, String> {
    match kind {
        CacheBackendKind::Redis => {
            redis_url.ok_or_else(|| "redis cache backend requires REDIS_URL to be set".to_string())
        }
        CacheBackendKind::Valkey => valkey_url.or(redis_url).ok_or_else(|| {
            "valkey cache backend requires VALKEY_URL or REDIS_URL to be set".to_string()
        }),
        CacheBackendKind::Local => Err(
            "local cache backends are test-only and cannot back a live shared runtime".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redis_backend_requires_an_explicit_url() {
        let error = cache_backend_url(CacheBackendKind::Redis, None, None).unwrap_err();
        assert_eq!(error, "redis cache backend requires REDIS_URL to be set");
    }

    #[test]
    fn valkey_backend_can_use_either_explicit_url() {
        assert_eq!(
            cache_backend_url(
                CacheBackendKind::Valkey,
                Some("redis://redis.internal/".to_string()),
                None
            )
            .unwrap(),
            "redis://redis.internal/"
        );
        assert_eq!(
            cache_backend_url(
                CacheBackendKind::Valkey,
                None,
                Some("redis://valkey.internal/".to_string())
            )
            .unwrap(),
            "redis://valkey.internal/"
        );
    }

    #[test]
    fn local_backend_is_not_supported_for_live_shared_runtime() {
        let error = cache_backend_url(CacheBackendKind::Local, None, None).unwrap_err();
        assert_eq!(
            error,
            "local cache backends are test-only and cannot back a live shared runtime"
        );
    }
}
