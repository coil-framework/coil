use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CacheEntry, CacheKey, CacheModelError, FillDecision, FillLease, InvalidationSet,
    RequestCoalescingMode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CacheRepository {
    entries: BTreeMap<CacheKey, CacheEntry>,
    tag_index: BTreeMap<crate::InvalidationTag, BTreeSet<CacheKey>>,
    inflight_fills: BTreeMap<CacheKey, String>,
}

impl CacheRepository {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn insert(&mut self, entry: CacheEntry) {
        let key = entry.key.clone();
        if let Some(previous) = self.entries.insert(key.clone(), entry) {
            self.detach_tags(&key, &previous.tags);
        }
        let tags = self.entries.get(&key).map(|entry| entry.tags.clone());
        if let Some(tags) = tags {
            self.attach_tags(&key, &tags);
        }
    }

    pub(crate) fn lookup(&self, key: &CacheKey) -> Option<CacheEntry> {
        self.entries.get(key).cloned()
    }

    pub(crate) fn remove(&mut self, key: &CacheKey) -> Option<CacheEntry> {
        let removed = self.entries.remove(key);
        if let Some(entry) = &removed {
            self.detach_tags(key, &entry.tags);
        }
        removed
    }

    pub(crate) fn invalidate(&mut self, tags: &InvalidationSet) -> Vec<CacheKey> {
        if tags.is_empty() {
            return Vec::new();
        }

        let mut removed = BTreeSet::new();
        for tag in tags.iter() {
            if let Some(keys) = self.tag_index.get(tag) {
                removed.extend(keys.iter().cloned());
            }
        }

        let removed = removed.into_iter().collect::<Vec<_>>();
        for key in &removed {
            let _ = self.remove(key);
        }
        removed
    }

    pub(crate) fn begin_fill(
        &mut self,
        key: &CacheKey,
        mode: RequestCoalescingMode,
        holder: impl Into<String>,
    ) -> FillDecision {
        let holder = holder.into();
        if mode == RequestCoalescingMode::Disabled {
            return FillDecision::Uncoalesced;
        }

        if let Some(existing) = self.inflight_fills.get(key) {
            return FillDecision::Coalesced {
                key: key.clone(),
                holder: existing.clone(),
            };
        }

        self.inflight_fills.insert(key.clone(), holder.clone());
        FillDecision::Start(FillLease {
            key: key.clone(),
            holder,
        })
    }

    pub(crate) fn complete_fill(&mut self, lease: &FillLease) -> Result<(), CacheModelError> {
        match self.inflight_fills.remove(&lease.key) {
            Some(_) => Ok(()),
            None => Err(CacheModelError::UnknownInflightFill {
                key: lease.key.to_string(),
            }),
        }
    }

    fn attach_tags(&mut self, key: &CacheKey, tags: &InvalidationSet) {
        for tag in tags.iter() {
            self.tag_index
                .entry(tag.clone())
                .or_default()
                .insert(key.clone());
        }
    }

    fn detach_tags(&mut self, key: &CacheKey, tags: &InvalidationSet) {
        for tag in tags.iter() {
            if let Some(keys) = self.tag_index.get_mut(tag) {
                keys.remove(key);
                if keys.is_empty() {
                    self.tag_index.remove(tag);
                }
            }
        }
    }
}
