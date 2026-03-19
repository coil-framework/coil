use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic write sequence for metadata audit executions.
///
/// This is a request-path-only counter used to report record ordering without
/// consulting the persisted audit table. Snapshot/reporting code can still
/// query the table when a true count is needed.
#[derive(Debug, Clone, Default)]
pub(super) struct MetadataWriteSequence {
    inner: Arc<AtomicU64>,
}

impl MetadataWriteSequence {
    pub(super) fn new() -> Self {
        Self {
            inner: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn next(&self) -> usize {
        self.inner.fetch_add(1, Ordering::Relaxed) as usize + 1
    }
}
