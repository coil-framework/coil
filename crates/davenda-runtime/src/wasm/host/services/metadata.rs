use std::sync::{Arc, Mutex};

use davenda_wasm::{MetadataExecution, MetadataGrant};

use super::super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct RuntimeMetadataBackend {
    entries: Arc<Mutex<Vec<MetadataWriteRecord>>>,
}

impl RuntimeMetadataBackend {
    pub(super) fn record(
        &self,
        kind: MetadataGrant,
        context: &InvocationContext,
    ) -> Result<MetadataExecution, String> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "metadata backend is poisoned".to_string())?;
        entries.push(MetadataWriteRecord {
            kind,
            trace_id: context.trace.trace_id.clone(),
            app_id: context.customer_app.app_id.clone(),
        });

        Ok(MetadataExecution {
            kind,
            recorded: true,
            journal_entries: entries.len(),
        })
    }

    #[cfg(test)]
    pub(super) fn records(&self) -> Vec<MetadataWriteRecord> {
        self.entries
            .lock()
            .expect("metadata backend poisoned")
            .clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetadataWriteRecord {
    pub kind: MetadataGrant,
    pub trace_id: String,
    pub app_id: String,
}
