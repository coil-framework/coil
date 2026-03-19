use std::fmt;

use super::planning::TlsRuntime;
use super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};
use super::state::TlsAutomationState;
use crate::{CertificateId, CertificateRecord, TlsInstant, TlsModelError};

#[cfg(test)]
mod file;
mod memory;
mod shared;

#[cfg(test)]
pub(super) use file::FileTlsAutomationBackend;
pub(super) use memory::MemoryTlsAutomationBackend;
#[cfg(test)]
pub(crate) fn test_state_path(scope: impl Into<String>) -> std::path::PathBuf {
    file::test_state_path(scope)
}
pub use shared::SharedTlsAutomationBackend;

pub trait TlsAutomationBackend: fmt::Debug + Send + Sync {
    fn snapshot(&self) -> TlsAutomationState;
    fn import_certificate(&self, record: CertificateRecord) -> Result<(), TlsModelError>;
    fn queue_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        now: TlsInstant,
    ) -> Result<RenewalPlan, TlsModelError>;
    fn begin_renewal(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement_certificate_id: CertificateId,
    ) -> Result<ChallengeTicket, TlsModelError>;
    fn fail_renewal(
        &self,
        certificate_id: &CertificateId,
    ) -> Result<CertificateRecord, TlsModelError>;
    fn activate_replacement(
        &self,
        runtime: &TlsRuntime,
        certificate_id: &CertificateId,
        replacement: CertificateRecord,
    ) -> Result<HotReloadEvent, TlsModelError>;
}
