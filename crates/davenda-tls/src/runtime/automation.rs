use std::sync::Arc;

use super::backend::{
    FileTlsAutomationBackend, MemoryTlsAutomationBackend, TlsAutomationBackend, default_state_path,
};
use super::planning::TlsRuntime;
use super::state::{CertificateInventory, TlsAutomationState};
use crate::{
    CertificateId, CertificateRecord, ChallengeTicket, HotReloadEvent, RenewalPlan, TlsInstant,
    TlsModelError,
};

#[derive(Debug, Clone)]
pub struct TlsAutomationRuntime {
    runtime: TlsRuntime,
    backend: Arc<dyn TlsAutomationBackend>,
}

impl TlsAutomationRuntime {
    pub fn new(runtime: TlsRuntime) -> Self {
        Self::with_backend(runtime, Arc::new(MemoryTlsAutomationBackend::new()))
    }

    pub fn ephemeral(runtime: TlsRuntime) -> Self {
        Self::with_backend(runtime, Arc::new(MemoryTlsAutomationBackend::new()))
    }

    pub fn with_persistent_backend(runtime: TlsRuntime, scope: impl Into<String>) -> Self {
        Self::with_backend(
            runtime,
            Arc::new(FileTlsAutomationBackend::new(default_state_path(
                scope.into(),
            ))),
        )
    }

    pub fn with_backend(runtime: TlsRuntime, backend: Arc<dyn TlsAutomationBackend>) -> Self {
        Self { runtime, backend }
    }

    pub fn snapshot(&self) -> TlsAutomationState {
        self.backend.snapshot()
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
        self.backend.import_certificate(record)
    }

    pub fn queue_renewal(
        &self,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError> {
        self.backend
            .queue_renewal(&self.runtime, certificate_id, now)
    }

    pub fn begin_renewal(
        &self,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError> {
        self.backend
            .begin_renewal(&self.runtime, certificate_id, replacement_certificate_id)
    }

    pub fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError> {
        self.backend.fail_renewal(certificate_id)
    }

    pub fn activate_replacement(
        &self,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError> {
        self.backend
            .activate_replacement(&self.runtime, certificate_id, replacement)
    }

    pub fn backend(&self) -> &dyn TlsAutomationBackend {
        self.backend.as_ref()
    }
}
