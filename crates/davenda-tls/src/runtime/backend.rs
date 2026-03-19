use std::fmt;

use super::planning::TlsRuntime;
use super::planning::{ChallengeTicket, HotReloadEvent, RenewalPlan};
use super::state::TlsAutomationState;
use crate::{CertificateId, CertificateRecord, TlsInstant, TlsModelError};

mod memory;
mod shared;
#[cfg(test)]
mod testing;

pub(super) use memory::MemoryTlsAutomationBackend;
#[cfg(test)]
pub(crate) use testing::TestFileTlsAutomationBackend;
pub use shared::SharedTlsAutomationBackend;
#[cfg(test)]
pub(crate) use testing::test_file_state_path;

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
