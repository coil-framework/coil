use std::sync::Arc;

use super::backend::{PostgresTlsControlPlaneStore, TlsControlPlaneStore};
use super::planning::TlsRuntime;
use super::state::{CertificateInventory, TlsControlPlaneState};
use crate::{
    CertificateId, CertificateRecord, ChallengeTicket, HotReloadEvent, RenewalPlan, TlsInstant,
    TlsModelError,
};
use davenda_data::DataRuntime;

#[derive(Debug, Clone)]
pub struct TlsControlPlaneRuntime {
    runtime: TlsRuntime,
    store: Arc<dyn TlsControlPlaneStore>,
}

impl TlsControlPlaneRuntime {
    #[cfg(test)]
    pub fn new(runtime: TlsRuntime) -> Self {
        Self::in_memory_control_plane_for_tests(runtime)
    }

    pub fn in_memory_control_plane_for_tests(runtime: TlsRuntime) -> Self {
        Self::with_store(
            runtime,
            Arc::new(super::backend::MemoryTlsControlPlaneStore::new()),
        )
    }

    #[cfg(test)]
    pub fn with_test_persistence_control_plane_for_tests(
        runtime: TlsRuntime,
        scope: impl Into<String>,
    ) -> Self {
        Self::with_store(
            runtime,
            Arc::new(super::backend::TestPersistenceTlsControlPlaneStore::new(
                super::backend::test_persistence_state_path(scope),
            )),
        )
    }

    pub fn with_distributed_postgres_control_plane(
        runtime: TlsRuntime,
        data_runtime: &DataRuntime,
        namespace: impl Into<String>,
    ) -> Result<Self, TlsModelError> {
        Ok(Self::with_store(
            runtime,
            Arc::new(PostgresTlsControlPlaneStore::new(data_runtime, namespace)?),
        ))
    }

    fn with_store(runtime: TlsRuntime, store: Arc<dyn TlsControlPlaneStore>) -> Self {
        Self { runtime, store }
    }

    pub fn snapshot(&self) -> TlsControlPlaneState {
        self.store.snapshot()
    }

    pub fn inventory(&self) -> CertificateInventory {
        self.snapshot().inventory
    }

    pub fn renewal_queue(&self) -> Vec<RenewalPlan> {
        self.snapshot().renewal_queue
    }

    pub fn pending_challenges(&self) -> Vec<ChallengeTicket> {
        self.snapshot().pending_challenges
    }

    pub fn hot_reload_events(&self) -> Vec<HotReloadEvent> {
        self.snapshot().hot_reload_events
    }

    pub fn import_certificate(&self, record: CertificateRecord) -> Result<(), TlsModelError> {
        self.store.import_certificate(record)
    }

    pub fn queue_renewal(
        &self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        self.store.queue_renewal(&self.runtime, certificate_id, now)
    }

    pub fn begin_renewal(
        &self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        self.store
            .begin_renewal(&self.runtime, certificate_id, replacement_certificate_id)
    }

    pub fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.store.fail_renewal(certificate_id)
    }

    pub fn activate_replacement(
        &self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        self.store
            .activate_replacement(&self.runtime, certificate_id, replacement)
    }

    pub fn control_plane_store(&self) -> &dyn TlsControlPlaneStore {
        self.store.as_ref()
    }
}
