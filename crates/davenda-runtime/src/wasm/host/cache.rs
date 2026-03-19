use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(super) struct CompiledModuleCache<K, V> {
    entries: Mutex<HashMap<K, Arc<V>>>,
}

impl<K, V> Default for CompiledModuleCache<K, V>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }
}

impl<K, V> CompiledModuleCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub(super) fn get_or_insert_with<E, F>(
        &self,
        key: K,
        compile: F,
    ) -> Result<Arc<V>, E>
    where
        F: FnOnce() -> Result<V, E>,
    {
        if let Some(module) = self
            .entries
            .lock()
            .expect("compiled module cache poisoned")
            .get(&key)
            .cloned()
        {
            return Ok(module);
        }

        let compiled = Arc::new(compile()?);
        let mut entries = self
            .entries
            .lock()
            .expect("compiled module cache poisoned");
        Ok(entries.entry(key).or_insert_with(|| compiled.clone()).clone())
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries
            .lock()
            .expect("compiled module cache poisoned")
            .len()
    }
}

#[cfg(test)]
mod tests {
    use super::CompiledModuleCache;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn compiled_module_cache_reuses_cached_values() {
        let cache = CompiledModuleCache::<String, u32>::default();
        let compiles = AtomicUsize::new(0);

        let first = cache
            .get_or_insert_with("artifact-a".to_string(), || {
                compiles.fetch_add(1, Ordering::SeqCst);
                Ok::<u32, ()>(41_u32)
            })
            .expect("cache insert succeeds");
        let second = cache
            .get_or_insert_with("artifact-a".to_string(), || {
                compiles.fetch_add(1, Ordering::SeqCst);
                Ok::<u32, ()>(99_u32)
            })
            .expect("cache hit succeeds");

        assert_eq!(cache.len(), 1);
        assert_eq!(compiles.load(Ordering::SeqCst), 1);
        assert_eq!(*first, 41);
        assert_eq!(*second, 41);
    }

    #[test]
    fn compiled_module_cache_distinguishes_keys() {
        let cache = CompiledModuleCache::<String, u32>::default();

        let first = cache
            .get_or_insert_with("artifact-a".to_string(), || Ok::<u32, ()>(41_u32))
            .expect("cache insert succeeds");
        let second = cache
            .get_or_insert_with("artifact-b".to_string(), || Ok::<u32, ()>(42_u32))
            .expect("cache insert succeeds");

        assert_eq!(cache.len(), 2);
        assert_eq!(*first, 41);
        assert_eq!(*second, 42);
    }
}
